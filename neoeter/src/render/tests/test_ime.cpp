// T1.6 — SDL text input state machine.
//
// The tests drive CIME with the same events SDL delivers (TEXTEDITING and
// TEXTINPUT) and check the buffer the Python `ime` module reads. No X server,
// IME or window is needed.

#include <support/TestAssert.h>

#include <Neoeter/Render/IME.h>

#include <string>

class TestIME : public CIME
{
public:
	using CIME::DecCurPos;
	using CIME::DelCurPos;
	using CIME::OnChar;
	using CIME::WMChar;
};

static std::string CurrentText()
{
	std::string text;
	CIME ime;
	ime.GetText(text);
	return text;
}

int main()
{
	CIME::Clear();
	TEST_ASSERT(CurrentText().empty());
	TEST_ASSERT(CIME::GetCompLen() == 0);

	// Composition: the OS IME is building "ni" (start 0, length 2).
	CIME::OnSDLTextEditing("ni", 0, 2);
	TEST_ASSERT(CIME::GetCompLen() == 2);
	TEST_ASSERT(CIME::GetULBegin() == 0);
	TEST_ASSERT(CIME::GetULEnd() == 2);
	TEST_ASSERT(CurrentText().empty());

	// Commit: SDL sends the finished UTF-8 text and clears the composition.
	CIME::OnSDLTextInput("你");
	TEST_ASSERT_MSG(CurrentText() == "你", "committed UTF-8 text");
	TEST_ASSERT(CIME::GetCompLen() == 0);

	// Composition replaced by a second edit, then committed.
	CIME::OnSDLTextEditing("hao", 0, 3);
	TEST_ASSERT(CIME::GetCompLen() == 3);
	CIME::OnSDLTextInput("好");
	TEST_ASSERT_MSG(CurrentText() == "你好", "text accumulates");

	// SetText / cursor editing (the Python ime.SetText / Delete path).
	TestIME ime;
	ime.SetText("abc", 3);
	TEST_ASSERT(CurrentText() == "abc");
	ime.DecCurPos();
	ime.DecCurPos();
	ime.DelCurPos();
	TEST_ASSERT_MSG(CurrentText() == "ac", "delete before cursor");

	// Number mode filters non-digits.
	CIME::Clear();
	ime.SetNumberMode();
	ime.OnChar(L'7');
	TEST_ASSERT(CurrentText() == "7");
	ime.OnChar(L'x');
	TEST_ASSERT(CurrentText() == "7");
	ime.SetStringMode();

	// Editing keys still arrive as WM_CHAR on the SDL path.
	CIME::Clear();
	TestIME edit;
	edit.EnableCaptureInput();
	edit.SetText("abc", 3);
	edit.WMChar(nullptr, 0x0102, 8, 0);
	TEST_ASSERT_MSG(CurrentText() == "ab", "backspace deletes before the cursor");
	edit.DecCurPos();
	edit.WMChar(nullptr, 0x0102, 127, 0);
	TEST_ASSERT_MSG(CurrentText() == "a", "delete removes at the cursor");
	edit.DisableCaptureInput();

	// Candidate/reading APIs are empty but safe (SDL has no candidate window).
	std::string candidate;
	TEST_ASSERT(ime.GetCandidateCount() == 0);
	TEST_ASSERT(ime.GetCandidate(0, candidate) == 0 && candidate.empty());
	TEST_ASSERT(ime.GetReadingError() == 0);

	CIME::Clear();
	return 0;
}
