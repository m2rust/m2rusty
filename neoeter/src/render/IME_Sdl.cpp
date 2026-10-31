// SDL-backed IME (T1.6).
//
// The Windows IMM/TSF implementation in IME.cpp is compiled out when
// OPENMT2_SDL_WINDOW is defined (both targets). SDL delivers the platform IME
// as SDL_TEXTEDITING (composition) and SDL_TEXTINPUT (committed text); this
// file turns those events into the text buffer the Python `ime` module reads.
//
// Limitation: SDL does not expose the candidate list or the reading window, so
// those APIs return empty results. The OS IME window is still shown by SDL on
// platforms that provide one (Windows, IBus/Fcitx on Linux).

#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/IME.h>
#include <Neoeter/Platform/Base/Utils.h>

#if defined(OPENMT2_SDL_WINDOW)

#include <SDL2/SDL.h>

bool CIME::ms_bInitialized = false;
bool CIME::ms_bDisableIMECompletely = false;
bool CIME::ms_bUILessMode = false;
bool CIME::ms_bImeEnabled = false;
bool CIME::ms_bCaptureInput = false;
bool CIME::ms_bChineseIME = false;
bool CIME::ms_bUseIMMCandidate = false;

HWND CIME::ms_hWnd = nullptr;
HKL CIME::ms_hklCurrent = nullptr;
char CIME::ms_szKeyboardLayout[KL_NAMELENGTH + 1] = {};
OSVERSIONINFOA CIME::ms_stOSVI = {};

HINSTANCE CIME::ms_hImm32Dll = nullptr;
HINSTANCE CIME::ms_hCurrentImeDll = nullptr;
DWORD CIME::ms_dwImeState = 0;
DWORD CIME::ms_adwId[2] = {0, 0};

DWORD CIME::ms_dwIMELevel = 3;
DWORD CIME::ms_dwIMELevelSaved = 3;

bool CIME::ms_bCandidateList = false;
DWORD CIME::ms_dwCandidateCount = 0;
bool CIME::ms_bVerticalCandidate = false;
int CIME::ms_iCandListIndexBase = 0;
WCHAR CIME::ms_wszCandidate[CIME::MAX_CANDLIST][CIME::MAX_CANDIDATE_LENGTH] = {};
DWORD CIME::ms_dwCandidateSelection = 0;
DWORD CIME::ms_dwCandidatePageSize = 0;

bool CIME::ms_bReadingInformation = false;
int CIME::ms_iReadingError = 0;
bool CIME::ms_bHorizontalReading = false;
std::vector<wchar_t> CIME::ms_wstrReading;

wchar_t* CIME::ms_wszCurrentIndicator = nullptr;

IIMEEventSink* CIME::ms_pEvent = nullptr;

wchar_t CIME::m_wText[IMESTR_MAXLEN] = {};
int CIME::ms_compLen = 0;
int CIME::ms_curpos = 0;
int CIME::ms_lastpos = 0;
int CIME::ms_ulbegin = 0;
int CIME::ms_ulend = 0;

UINT CIME::ms_uOutputCodePage = 65001;
UINT CIME::ms_uInputCodePage = 65001;

std::string CIME::m_stText;
std::string CIME::m_stComposition;
CIME* CIME::ms_pSdlInstance = nullptr;

// m_stText holds UTF-8 and ms_curpos / ms_lastpos are byte offsets; these keep
// the cursor on code point boundaries.
static bool IsUtf8Continuation(unsigned char c) { return (c & 0xC0) == 0x80; }

static size_t Utf8Length(const std::string& s)
{
	size_t n = 0;
	for (unsigned char c : s)
		if (!IsUtf8Continuation(c))
			++n;
	return n;
}

// Decodes one code point at s[i], advancing i. Invalid bytes map to U+FFFD.
static wchar_t DecodeUtf8(const std::string& s, size_t& i)
{
	const unsigned char c = (unsigned char)s[i++];
	int extra = 0;
	unsigned int cp = c;
	if (c >= 0xF0) { extra = 3; cp = c & 0x07; }
	else if (c >= 0xE0) { extra = 2; cp = c & 0x0F; }
	else if (c >= 0xC0) { extra = 1; cp = c & 0x1F; }
	else if (c >= 0x80) return 0xFFFD;
	for (; extra > 0; --extra) {
		if (i >= s.size() || !IsUtf8Continuation((unsigned char)s[i]))
			return 0xFFFD;
		cp = (cp << 6) | ((unsigned char)s[i++] & 0x3F);
	}
	return (wchar_t)cp;
}

CIME::CIME()
	: m_hOrgIMC(nullptr)
	, m_max(IMESTR_MAXLEN)
	, m_userMax(IMESTR_MAXLEN)
	, m_bOnlyNumberMode(false)
	, m_bEnablePaste(true)
	, m_bUseDefaultIME(false)
{
	ms_pSdlInstance = this;
}

CIME::~CIME()
{
	if (ms_pSdlInstance == this)
		ms_pSdlInstance = nullptr;
}

bool CIME::Initialize(HWND hWnd)
{
	ms_hWnd = hWnd;
	ms_bInitialized = true;
	ms_bImeEnabled = true;
	ms_dwImeState = 1;
	Clear();
	SDL_StartTextInput();
	return true;
}

void CIME::Uninitialize()
{
	SDL_StopTextInput();
	ms_bImeEnabled = false;
	ms_bInitialized = false;
	ms_hWnd = nullptr;
}

void CIME::Clear()
{
	memset(m_wText, 0, sizeof(m_wText));
	ms_compLen = 0;
	ms_curpos = 0;
	ms_lastpos = 0;
	ms_ulbegin = 0;
	ms_ulend = 0;
	m_stText.clear();
	m_stComposition.clear();
}

void CIME::SetMax(int iMax) { m_max = iMax; }
void CIME::SetUserMax(int iMax) { m_userMax = iMax; }

void CIME::SetText(const char* c_szText, int len)
{
	if (!c_szText)
		return;
	m_stText.assign(c_szText, len < 0 ? strlen(c_szText) : (size_t)len);
	ms_curpos = (int)m_stText.size();
	ms_lastpos = ms_curpos;
}

int CIME::GetText(std::string& rstrText, bool /*addCodePage*/)
{
	rstrText.append(m_stText);
	return (int)m_stText.size();
}

const char* CIME::GetCodePageText() { return ""; }
int CIME::GetCodePage() { return (int)ms_uOutputCodePage; }

int CIME::GetCandidateCount() { return 0; }
int CIME::GetCandidatePageCount() { return 0; }
int CIME::GetCandidate(DWORD, std::string&) { return 0; }
int CIME::GetCandidateSelection() { return 0; }
int CIME::GetReading(std::string&) { return 0; }
int CIME::GetReadingError() { return 0; }

void CIME::SetInputMode(DWORD dwMode) { ms_dwImeState = dwMode; }
DWORD CIME::GetInputMode() { return ms_dwImeState; }

bool CIME::IsIMEEnabled() { return ms_bImeEnabled; }

void CIME::EnableIME(bool bEnable)
{
	// IME enablement controls composition/candidates, not SDL's text events.
	// Stopping text input here made fields with enable_ime=0 (login id/password)
	// receive no SDL_TEXTINPUT at all, so typing produced nothing. Text input
	// stays active for the lifetime of the IME host; Uninitialize stops it.
	ms_bImeEnabled = bEnable;
}

void CIME::DisableIME() { EnableIME(false); }

void CIME::EnableCaptureInput() { ms_bCaptureInput = true; }
void CIME::DisableCaptureInput() { ms_bCaptureInput = false; }
bool CIME::IsCaptureEnabled() { return ms_bCaptureInput; }

void CIME::SetNumberMode() { m_bOnlyNumberMode = true; }
void CIME::SetStringMode() { m_bOnlyNumberMode = false; }

bool CIME::__IsWritable(wchar_t key)
{
	for (wchar_t except : m_exceptKey)
		if (except == key)
			return false;
	return true;
}

void CIME::AddExceptKey(wchar_t key) { m_exceptKey.push_back(key); }
void CIME::ClearExceptKey() { m_exceptKey.clear(); }

void CIME::PasteTextFromClipBoard()
{
	char* clipboard = SDL_GetClipboardText();
	if (clipboard)
	{
		PasteString(clipboard);
		SDL_free(clipboard);
	}
}

void CIME::EnablePaste(bool bFlag) { m_bEnablePaste = bFlag; }

void CIME::PasteString(const char* str)
{
	if (!m_bEnablePaste || !str)
		return;
	m_stText.append(str);
	ms_curpos = ms_lastpos = (int)m_stText.size();
}

void CIME::FinalizeString(bool /*bSend*/) { ms_compLen = 0; }

void CIME::UseDefaultIME() { m_bUseDefaultIME = true; }

int CIME::GetCurPos() { return ms_curpos; }
int CIME::GetCompLen() { return ms_compLen; }
int CIME::GetULBegin() { return ms_ulbegin; }
int CIME::GetULEnd() { return ms_ulend; }

void CIME::CloseCandidateList() { ms_bCandidateList = false; ms_dwCandidateCount = 0; }
void CIME::CloseReadingInformation() { ms_bReadingInformation = false; }
void CIME::ChangeInputLanguage() {}
void CIME::ChangeInputLanguageWorker() {}

LRESULT CIME::WMInputLanguage(HWND, UINT, WPARAM, LPARAM) { return 0; }
LRESULT CIME::WMStartComposition(HWND, UINT, WPARAM, LPARAM) { return 0; }
LRESULT CIME::WMComposition(HWND, UINT, WPARAM, LPARAM) { return 0; }
LRESULT CIME::WMEndComposition(HWND, UINT, WPARAM, LPARAM) { return 0; }
LRESULT CIME::WMNotify(HWND, UINT, WPARAM, LPARAM) { return 0; }
LRESULT CIME::WMChar(HWND, UINT, WPARAM wParam, LPARAM lParam)
{
	// Printable characters arrive as committed UTF-8 through SDL_TEXTINPUT
	// (OnSDLTextInput). WM_CHAR is still used for editing keys and the
	// OnWM_CHAR control path (return/tab/escape).
	const unsigned char c = (unsigned char)(wParam & 0xff);
	if (c == 8 || c == 127)
	{
		if (!ms_bCaptureInput)
			return 0;
		if (c == 8)
		{
			if (ms_curpos > 0)
			{
				DecCurPos();
				DelCurPos();
			}
		}
		else
		{
			DelCurPos();
		}
		if (ms_pEvent)
			ms_pEvent->OnUpdate();
		return 0;
	}
	if (ms_pEvent && ms_pEvent->OnWM_CHAR(wParam, lParam))
		return 0;
	return 0;
}

void CIME::IncCurPos()
{
	if (ms_curpos < ms_lastpos)
		++ms_curpos;
	while (ms_curpos < ms_lastpos && IsUtf8Continuation((unsigned char)m_stText[ms_curpos]))
		++ms_curpos;
}
void CIME::DecCurPos()
{
	if (ms_curpos > 0)
		--ms_curpos;
	while (ms_curpos > 0 && IsUtf8Continuation((unsigned char)m_stText[ms_curpos]))
		--ms_curpos;
}
void CIME::SetCurPos(int offset)
{
	// As on Windows: out of range (EditLine passes -1 on focus) means the end.
	// In range, offset is a rendered character index, not a byte offset.
	ms_lastpos = (int)m_stText.size();
	if (offset < 0)
	{
		ms_curpos = ms_lastpos;
		return;
	}
	size_t pos = 0;
	for (int n = 0; n < offset && pos < m_stText.size(); ++n)
	{
		++pos;
		while (pos < m_stText.size() && IsUtf8Continuation((unsigned char)m_stText[pos]))
			++pos;
	}
	ms_curpos = (int)pos;
}
void CIME::DelCurPos()
{
	// Delete key semantics: remove the character at the cursor, not before it.
	if (ms_curpos < ms_lastpos)
	{
		size_t end = (size_t)ms_curpos + 1;
		while (end < m_stText.size() && IsUtf8Continuation((unsigned char)m_stText[end]))
			++end;
		m_stText.erase(ms_curpos, end - ms_curpos);
		ms_lastpos = (int)m_stText.size();
	}
}

void CIME::CheckInputLocale() {}
void CIME::CheckToggleState() {}
void CIME::SetSupportLevel(DWORD dwImeLevel) { ms_dwIMELevel = dwImeLevel; }

void CIME::InsertString(wchar_t* /*szString*/, int /*iSize*/) {}
void CIME::OnChar(wchar_t c)
{
	if (m_bOnlyNumberMode && !iswdigit(c))
		return;
	if (!__IsWritable(c))
		return;
	m_stText.push_back((char)c);
	ms_curpos = ms_lastpos = (int)m_stText.size();
}

UINT CIME::GetCodePageFromLang(LANGID) { return 65001; }
void CIME::ResultProcess(HIMC) {}
void CIME::CompositionProcessBuilding(HIMC) {}
void CIME::CompositionProcess(HIMC) {}
void CIME::AttributeProcess(HIMC) {}
void CIME::CandidateProcess(HIMC) {}
void CIME::ReadingProcess(HIMC) {}
bool CIME::IsMax(const wchar_t*, int) { return false; }
DWORD CIME::GetImeId(UINT) { return 0; }
bool CIME::GetReadingWindowOrientation() { return false; }
void CIME::SetupImeApi() {}

void CIME::OnSDLTextEditing(const char* utf8, int start, int length)
{
	m_stComposition = utf8 ? utf8 : "";
	ms_compLen = (int)m_stComposition.size();
	ms_ulbegin = start;
	ms_ulend = start + length;
	if (ms_pEvent)
		ms_pEvent->OnUpdate();
}

void CIME::OnSDLTextInput(const char* utf8)
{
	if (!utf8)
		return;

	m_stComposition.clear();
	ms_compLen = 0;

	// Same gate as WMChar on Windows: with no focused edit line, keys are
	// hotkeys and must not pile up in the hidden text buffer.
	if (!ms_bCaptureInput)
		return;

	CIME* ime = ms_pSdlInstance;
	size_t maxChars = IMESTR_MAXLEN;
	if (ime && ime->m_max > 0 && (size_t)ime->m_max < maxChars)
		maxChars = (size_t)ime->m_max;

	const std::string text = utf8;
	std::string insert;
	size_t length = Utf8Length(m_stText);
	for (size_t i = 0; i < text.size();)
	{
		const size_t begin = i;
		const wchar_t c = DecodeUtf8(text, i);
		if (ime && ime->m_bOnlyNumberMode && !iswdigit(c))
			continue;
		if (ime && !ime->__IsWritable(c))
			continue;

		// '|' starts a colour/hyperlink tag, so a literal one is doubled.
		const int copies = (c == L'|') ? 2 : 1;
		for (int n = 0; n < copies && length < maxChars; ++n, ++length)
			insert.append(text, begin, i - begin);
	}
	if (insert.empty())
		return;

	if (ms_curpos < 0 || ms_curpos > (int)m_stText.size())
		ms_curpos = (int)m_stText.size();
	m_stText.insert((size_t)ms_curpos, insert);
	ms_curpos += (int)insert.size();
	ms_lastpos = (int)m_stText.size();
	if (ms_pEvent)
		ms_pEvent->OnUpdate();
}

#endif // OPENMT2_SDL_WINDOW
