// Reference dump for the Rust UI core (crates/m2-ui-core).
//
//   m2diff_ui scenario <file>   run a scenario script against the REAL window
//                               manager / window / slot classes
//   m2diff_ui text <file>       tag parser + text layout over a corpus
//
// How events are observed. Every window gets a real Python object as its
// handler (m_poHandler). The object defines a method only when the scenario
// says so ("handler ID Event RET"); a missing method is exactly what a null
// handler or a Python class without the method looks like to
// PyCallClassMemberFunc (it returns false), and RET is the Python return value
// the C++ code reacts to (e.g. OnMouseLeftButtonDown returning 1). Each call
// appends "ev <id> <Name> <args>" to a Python list that is flushed after every
// command. Nothing in the engine is replaced.
//
// Text metrics. The real CGraphicTextInstance::Update runs with a CGraphicText
// whose font texture has its glyph table pre-filled with deterministic fake
// metrics (see FakeInfo), so no font file and no GPU are needed. Render() is
// never called.
//
// Scenario syntax and output format: see crates/m2-ui-core/tests/support.

#include <Neoeter/Python/Bindings/StdAfx.h>
#include <Neoeter/Python/Bindings/PythonWindow.h>
#include <Neoeter/Python/Bindings/PythonSlotWindow.h>
#include <Neoeter/Python/Bindings/PythonGridSlotWindow.h>
#include <Neoeter/Python/Bindings/PythonWindowManager.h>
#include <Neoeter/Python/Host/PythonLauncher.h>
#include <Neoeter/Python/Host/PythonUtils.h>
#include <Neoeter/Python/Host/Resource.h>
#include <Neoeter/Render/GrpText.h>
#include <Neoeter/Render/GrpTextInstance.h>
#include <Neoeter/Render/GrpExpandedImageInstance.h>
#include <Neoeter/Render/GrpMarkInstance.h>
#include <Neoeter/Render/TextTag.h>
#include <Neoeter/Render/Util.h>
#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/Timer.h>
#include <Neoeter/Platform/Locale/StringCodec.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iostream>
#include <map>
#include <sstream>
#include <string>
#include <vector>

using namespace UI;

namespace
{

std::map<int, CWindow *> g_byId;
std::map<CWindow *, std::string> g_names;
PyObject *g_hClass = nullptr;
PyObject *g_flush = nullptr;
PyObject *g_define = nullptr;
PyObject *g_mouseHandler = nullptr;
CGraphicText *g_font = nullptr;

uint32_t FBits(float f)
{
    uint32_t u;
    std::memcpy(&u, &f, 4);
    return u;
}

std::string Hex(const std::string &s)
{
    if (s.empty())
        return "-";
    std::string o;
    char b[4];
    for (unsigned char c : s)
    {
        std::snprintf(b, sizeof b, "%02x", c);
        o += b;
    }
    return o;
}

std::string UnHex(const std::string &s)
{
    std::string o;
    if (s == "-")
        return o;
    for (size_t i = 0; i + 1 < s.size(); i += 2)
        o += static_cast<char>(std::strtol(s.substr(i, 2).c_str(), nullptr, 16));
    return o;
}

std::string WHex(const std::wstring &w)
{
    if (w.empty())
        return "-";
    std::string o;
    char b[16];
    for (size_t i = 0; i < w.size(); ++i)
    {
        std::snprintf(b, sizeof b, i ? ",%x" : "%x", static_cast<unsigned>(w[i]));
        o += b;
    }
    return o;
}

std::string NameOf(CWindow *w)
{
    if (!w)
        return "null";
    auto it = g_names.find(w);
    if (it != g_names.end())
        return it->second;
    return "?";
}

void FlushEvents()
{
    PyObject *r = PyObject_CallObject(g_flush, nullptr);
    if (!r)
    {
        PyErr_Print();
        return;
    }
    std::fputs(PyString_AsString(r), stdout);
    Py_DECREF(r);
}

const char *TypeName(CWindow *w)
{
    if (dynamic_cast<CGridSlotWindow *>(w))
        return "GridSlot";
    if (dynamic_cast<CSlotWindow *>(w))
        return "Slot";
    if (dynamic_cast<CTextLine *>(w))
        return "TextLine";
    if (dynamic_cast<CNumberLine *>(w))
        return "NumberLine";
    if (dynamic_cast<CMarkBox *>(w))
        return "MarkBox";
    if (dynamic_cast<CExpandedImageBox *>(w))
        return "ExpandedImageBox";
    if (dynamic_cast<CImageBox *>(w))
        return "ImageBox";
    if (dynamic_cast<CAniImageBox *>(w))
        return "AniImageBox";
    if (dynamic_cast<CDragButton *>(w))
        return "DragButton";
    if (dynamic_cast<CRadioButton *>(w))
        return "RadioButton";
    if (dynamic_cast<CToggleButton *>(w))
        return "ToggleButton";
    if (dynamic_cast<CButton *>(w))
        return "Button";
    if (dynamic_cast<CBox *>(w))
        return "Box";
    if (dynamic_cast<CBar3D *>(w))
        return "Bar3D";
    if (dynamic_cast<CBar *>(w))
        return "Bar";
    if (dynamic_cast<CLine *>(w))
        return "Line";
    if (dynamic_cast<CLayer *>(w))
        return "Layer";
    return "Window";
}

void NameInternals()
{
    for (auto &kv : g_byId)
    {
        auto *s = dynamic_cast<CSlotWindow *>(kv.second);
        if (!s)
            continue;
        for (auto &sl : s->m_SlotList)
        {
            std::string base = std::to_string(kv.first) + ".";
            std::string idx = std::to_string(sl.dwSlotNumber);
            if (sl.pCoverButton)
                g_names[(CWindow *)sl.pCoverButton] = base + "cover." + idx;
            if (sl.pSlotButton)
                g_names[(CWindow *)sl.pSlotButton] = base + "btn." + idx;
            if (sl.pSignImage)
                g_names[sl.pSignImage] = base + "sign." + idx;
            if (sl.pFinishCoolTimeEffect)
                g_names[sl.pFinishCoolTimeEffect] = base + "fx." + idx;
        }
    }
}

void DumpWindow(CWindow *w, int depth)
{
    std::printf("w %d %s %s show=%d pos=%ld,%ld size=%ld,%ld rect=%ld,%ld,%ld,%ld flag=%x ha=%d va=%d bias=%ld,%ld,%ld,%ld "
                "par=%s",
                depth, NameOf(w).c_str(), TypeName(w), w->IsShow() ? 1 : 0, w->m_x, w->m_y, w->m_lWidth, w->m_lHeight,
                w->m_rect.left, w->m_rect.top, w->m_rect.right, w->m_rect.bottom, w->m_dwFlag, (int)w->m_HorizontalAlign,
                (int)w->m_VerticalAlign, w->m_limitBiasRect.left, w->m_limitBiasRect.right, w->m_limitBiasRect.top,
                w->m_limitBiasRect.bottom, NameOf(w->m_pParent).c_str());
    if (auto *b = dynamic_cast<CButton *>(w))
        std::printf(" enable=%d pressed=%d flash=%d", b->m_bEnable, b->m_isPressed, b->m_isFlash);
    if (auto *d = dynamic_cast<CDragButton *>(w))
        std::printf(" restrict=%ld,%ld,%ld,%ld", d->m_restrictArea.left, d->m_restrictArea.top, d->m_restrictArea.right,
                    d->m_restrictArea.bottom);
    if (auto *a = dynamic_cast<CAniImageBox *>(w))
        std::printf(" ani=%d,%d,%d", a->m_bycurDelay, a->m_byDelay, a->m_bycurIndex);
    std::printf(" kids=%zu", w->m_pChildList.size());
    std::printf("\n");
    for (auto *c : w->m_pChildList)
        DumpWindow(c, depth + 1);
}

std::string ListNames(const std::list<CWindow *> &l)
{
    std::string o = "[";
    bool first = true;
    for (auto *w : l)
    {
        if (!first)
            o += ",";
        first = false;
        o += NameOf(w);
    }
    return o + "]";
}

void DumpMgr()
{
    CWindowManager &m = CWindowManager::Instance();
    std::printf("mgr mouse=%ld,%ld picked=%ld,%ld drag=%ld,%ld dragging=%d point=%s active=%s lock=%s left=%s "
                "right=%s middle=%s attaching=%d atype=%u aindex=%u aslot=%u asize=%u,%u locklist=%s activelist=%s "
                "pickalways=%s reserve=%s keycap=[",
                m.m_lMouseX, m.m_lMouseY, m.m_lPickedX, m.m_lPickedY, m.m_lDragX, m.m_lDragY, m.IsDragging(),
                NameOf(m.m_pPointWindow).c_str(), NameOf(m.m_pActiveWindow).c_str(), NameOf(m.m_pLockWindow).c_str(),
                NameOf(m.m_pLeftCaptureWindow).c_str(), NameOf(m.m_pRightCaptureWindow).c_str(),
                NameOf(m.m_pMiddleCaptureWindow).c_str(), m.m_bAttachingFlag, m.m_dwAttachingType, m.m_dwAttachingIndex,
                m.m_dwAttachingSlotNumber, (unsigned)m.m_byAttachingIconWidth, (unsigned)m.m_byAttachingIconHeight,
                ListNames(m.m_LockWindowList).c_str(), ListNames(m.m_ActiveWindowList).c_str(),
                ListNames(m.m_PickAlwaysWindowList).c_str(), ListNames(m.m_ReserveDeleteWindowList).c_str());
    bool first = true;
    for (auto &kv : m.m_KeyCaptureWindowMap)
    {
        std::printf("%s%d=%s", first ? "" : ",", kv.first, NameOf(kv.second).c_str());
        first = false;
    }
    std::printf("]\n");
}

void DumpSlot(CWindow *w)
{
    auto *s = dynamic_cast<CSlotWindow *>(w);
    if (!s)
        return;
    std::printf("slotwin %s type=%u style=%u tooltip=%x usemode=%d usable=%d selected=[", NameOf(w).c_str(),
                s->m_dwSlotType, s->m_dwSlotStyle, s->m_dwToolTipSlotNumber, s->m_isUseMode, s->m_isUsableItem);
    bool first = true;
    for (auto v : s->m_dwSelectedSlotIndexList)
    {
        std::printf("%s%u", first ? "" : ",", v);
        first = false;
    }
    std::printf("]");
    if (auto *g = dynamic_cast<CGridSlotWindow *>(w))
    {
        std::printf(" grid=%ux%u vec=[", g->m_dwxCount, g->m_dwyCount);
        first = true;
        for (auto *p : g->m_SlotVector)
        {
            std::printf("%s%u", first ? "" : ",", p->dwSlotNumber);
            first = false;
        }
        std::printf("]");
    }
    CSlotWindow::TSlot *p = nullptr;
    if (s->GetPickedSlotPointer(&p))
        std::printf(" picked=%u", p->dwSlotNumber);
    else
        std::printf(" picked=-");
    p = nullptr;
    if (s->GetSelectedSlotPointer(&p))
        std::printf(" selptr=%u", p->dwSlotNumber);
    else
        std::printf(" selptr=-");
    std::printf("\n");
    for (auto &sl : s->m_SlotList)
    {
        std::printf("s %u state=%x center=%x item=%d index=%u cool=%08x start=%08x active=%d pos=%d,%d cell=%d,%d "
                    "placed=%d,%d base=%d inst=%d num=%d cover=%d btn=%d sign=%d fx=%d\n",
                    sl.dwSlotNumber, sl.dwState, sl.dwCenterSlotNumber, sl.isItem, sl.dwItemIndex, FBits(sl.fCoolTime),
                    FBits(sl.fStartCoolTime), sl.bActive, sl.ixPosition, sl.iyPosition, sl.ixCellSize, sl.iyCellSize,
                    sl.byxPlacedItemSize, sl.byyPlacedItemSize, sl.bRenderBaseSlotImage, sl.pInstance != nullptr,
                    sl.pNumberLine != nullptr, sl.pCoverButton != nullptr, sl.pSlotButton != nullptr,
                    sl.pSignImage != nullptr, sl.pFinishCoolTimeEffect != nullptr);
    }
}

// ---- fake glyph metrics ---------------------------------------------------

struct FakeInfo
{
    static short Width(unsigned c) { return 4 + (c % 7); }
    static short Height(unsigned c) { return 10 + (c % 4); }
    static float Advance(unsigned c) { return Width(c) + 1 + ((c & 1) ? 0.5f : 0.0f); }
};

void Prefill(WORD cp, wchar_t c)
{
    auto &map = g_font->m_fontTexture.m_charInfoMap;
    CGraphicFontTexture::TCharacterKey key(cp, c);
    if (map.find(key) != map.end())
        return;
    CGraphicFontTexture::TCharacterInfomation i{};
    i.index = 0;
    i.width = FakeInfo::Width(c);
    i.height = FakeInfo::Height(c);
    i.advance = FakeInfo::Advance(c);
    map[key] = i;
}

void PrefillText(const std::string &text)
{
    UINT cp = GetDefaultCodePage();
    const char *begin = text.c_str();
    const char *end = begin + text.size();
    Prefill(cp, ' ');
    std::vector<wchar_t> buf(text.size() * 2 + 4);
    while (begin < end)
    {
        const char *token = FindToken(begin, end);
        int n = Ymir_MultiByteToWideChar(cp, 0, begin, token - begin, buf.data(), buf.size());
        Prefill(cp, '*');
        for (int i = 0; i < n; ++i)
            Prefill(cp, buf[i]);
        if (token < end)
        {
            cp = ReadToken(token);
            begin = token + 5;
        }
        else
            begin = token;
    }
}

void EnsureFont()
{
    if (g_font)
        return;
    g_font = new CGraphicText("fake.fnt");
    g_font->m_fontTexture.m_fontMap[0] = nullptr; // IsEmpty() == false
    g_font->AddReference();
}

void DumpTextInstance(const char *label, CGraphicTextInstance &ti)
{
    std::printf("%s w=%d h=%d lines=%d value=%s chars=", label, (int)ti.m_textWidth, (int)ti.m_textHeight,
                (int)ti.GetTextLineCount(), Hex(ti.m_stText).c_str());
    for (size_t i = 0; i < ti.m_pCharInfoVector.size(); ++i)
    {
        auto *c = ti.m_pCharInfoVector[i];
        std::printf("%s%d:%08x:%d:%x", i ? "," : "", (int)c->width, FBits(c->advance), (int)c->height,
                    ti.m_dwColorInfoVector[i]);
    }
    if (ti.m_pCharInfoVector.empty())
        std::printf("-");
    std::printf(" links=");
    for (size_t i = 0; i < ti.m_hyperlinkVector.size(); ++i)
    {
        auto &l = ti.m_hyperlinkVector[i];
        std::printf("%s%d:%d:%s", i ? ";" : "", (int)l.sx, (int)l.ex, WHex(l.text).c_str());
    }
    if (ti.m_hyperlinkVector.empty())
        std::printf("-");
    std::printf(" pix=");
    for (int p = 0; p < (int)ti.m_textWidth + 5; p += 3)
        std::printf("%s%d", p ? "," : "", ti.PixelPositionToCharacterPosition(p));
    std::printf(" tp=");
    for (DWORD i = 0; i <= ti.m_pCharInfoVector.size() + 1; ++i)
    {
        float x, y;
        ti.__GetTextPos(i, &x, &y);
        std::printf("%s%08x/%08x", i ? "," : "", FBits(x), FBits(y));
    }
    std::printf("\n");
}

// ---- scenario -------------------------------------------------------------

void Define(int id, const std::string &ev, const std::string &ret)
{
    PyObject *r = PyObject_CallFunction(g_define, const_cast<char *>("iss"), id, ev.c_str(), ret.c_str());
    if (!r)
        PyErr_Print();
    Py_XDECREF(r);
}

DWORD TypeOf(const std::string &t)
{
    static const std::map<std::string, DWORD> m = {
        {"slot", CWindowManager::WT_SLOT},
        {"gridslot", CWindowManager::WT_GRIDSLOT},
        {"textline", CWindowManager::WT_TEXTLINE},
        {"markbox", CWindowManager::WT_MARKBOX},
        {"imagebox", CWindowManager::WT_IMAGEBOX},
        {"expimagebox", CWindowManager::WT_EXP_IMAGEBOX},
        {"aniimagebox", CWindowManager::WT_ANI_IMAGEBOX},
        {"button", CWindowManager::WT_BUTTON},
        {"radiobutton", CWindowManager::WT_RATIOBUTTON},
        {"togglebutton", CWindowManager::WT_TOGGLEBUTTON},
        {"dragbutton", CWindowManager::WT_DRAGBUTTON},
        {"box", CWindowManager::WT_BOX},
        {"bar", CWindowManager::WT_BAR},
        {"line", CWindowManager::WT_LINE},
        {"bar3d", CWindowManager::WT_BAR3D},
        {"numline", CWindowManager::WT_NUMLINE},
    };
    auto it = m.find(t);
    return it == m.end() ? 0xffffffff : it->second;
}

void AddFlag(CWindow *w, const std::string &f)
{
    if (f == "movable")
        w->AddFlag(CWindow::FLAG_MOVABLE);
    else if (f == "limit")
        w->AddFlag(CWindow::FLAG_LIMIT);
    else if (f == "dragable")
        w->AddFlag(CWindow::FLAG_DRAGABLE);
    else if (f == "attach")
        w->AddFlag(CWindow::FLAG_ATTACH);
    else if (f == "restrict_x")
        w->AddFlag(CWindow::FLAG_RESTRICT_X);
    else if (f == "restrict_y")
        w->AddFlag(CWindow::FLAG_RESTRICT_Y);
    else if (f == "float")
        w->AddFlag(CWindow::FLAG_FLOAT);
    else if (f == "not_pick")
        w->AddFlag(CWindow::FLAG_NOT_PICK);
    else if (f == "ignore_size")
        w->AddFlag(CWindow::FLAG_IGNORE_SIZE);
    else if (f == "rtl")
        w->AddFlag(CWindow::FLAG_RTL);
    else if (f == "ltr")
        w->RemoveFlag(CWindow::FLAG_RTL);
}

void RunScenario(const char *path)
{
    std::ifstream in(path);
    std::string line;
    CWindowManager &wm = CWindowManager::Instance();
    NameInternals();
    while (std::getline(in, line))
    {
        if (line.empty() || line[0] == '#')
            continue;
        std::printf("> %s\n", line.c_str());
        std::istringstream ss(line);
        std::vector<std::string> a;
        std::string t;
        while (ss >> t)
            a.push_back(t);
        auto I = [&](size_t i) { return (long)std::strtol(a[i].c_str(), nullptr, 0); };
        auto F = [&](size_t i) { return (float)std::strtod(a[i].c_str(), nullptr); };
        auto W = [&](size_t i) -> CWindow * {
            auto it = g_byId.find((int)I(i));
            return it == g_byId.end() ? nullptr : it->second;
        };
        const std::string &c = a[0];
        CWindow *w = a.size() > 1 ? W(1) : nullptr;
        auto S = [&]() { return dynamic_cast<CSlotWindow *>(w); };
        if (c == "screen")
        {
            wm.SetScreenSize(I(1), I(2));
            wm.SetResolution(a.size() > 4 ? I(3) : I(1), a.size() > 4 ? I(4) : I(2));
        }
        else if (c == "new")
        {
            int id = (int)I(1);
            PyObject *h = PyObject_CallFunction(g_hClass, const_cast<char *>("i"), id);
            CWindow *nw = a[2] == "window" ? wm.RegisterWindow(h, a[3].c_str())
                                           : wm.RegisterTypeWindow(h, TypeOf(a[2]), a[3].c_str());
            g_byId[id] = nw;
            g_names[nw] = std::to_string(id);
        }
        else if (c == "parent")
            wm.SetParent(W(1), W(2));
        else if (c == "name" && w)
            w->SetName(a[2].c_str());
        else if (c == "pos" && w)
            w->SetPosition(I(2), I(3));
        else if (c == "size" && w)
            w->SetSize(I(2), I(3));
        else if (c == "halign" && w)
            w->CWindow::SetHorizontalAlign(I(2));
        else if (c == "valign" && w)
            w->CWindow::SetVerticalAlign(I(2));
        else if (c == "show" && w)
            w->Show();
        else if (c == "hide" && w)
            w->Hide();
        else if (c == "flag" && w)
            AddFlag(w, a[2]);
        else if (c == "bias" && w)
            w->SetLimitBias(I(2), I(3), I(4), I(5));
        else if (c == "updaterect" && w)
            w->UpdateRect();
        else if (c == "settop" && w)
            wm.SetTop(w);
        else if (c == "lock" && w)
            wm.LockWindow(w);
        else if (c == "unlock")
            wm.UnlockWindow();
        else if (c == "focus" && w)
            wm.ActivateWindow(w);
        else if (c == "killfocus")
        {
            if (w)
            {
                if (w == wm.GetActivateWindow())
                    wm.DeactivateWindow();
            }
            else
                wm.DeactivateWindow();
        }
        else if (c == "pickalways" && w)
            wm.SetPickAlways(w);
        else if (c == "destroy" && w)
        {
            wm.DestroyWindow(w);
            g_byId.erase((int)I(1));
        }
        else if (c == "handler" && w)
            Define((int)I(1), a[2], a[3]);
        else if (c == "mousehandler")
        {
            Define(-1, "DeattachObject", "0");
            PyObject *h = PyObject_CallFunction(g_hClass, const_cast<char *>("i"), -1);
            wm.SetMouseHandler(h);
        }
        else if (c == "update")
        {
            wm.Update();
            NameInternals();
        }
        else if (c == "tick")
            CTimer::Instance().Advance();
        else if (c == "move")
            wm.RunMouseMove(I(1), I(2));
        else if (c == "ldown")
            wm.RunMouseLeftButtonDown(I(1), I(2));
        else if (c == "lup")
            wm.RunMouseLeftButtonUp(I(1), I(2));
        else if (c == "ldbl")
            wm.RunMouseLeftButtonDoubleClick(I(1), I(2));
        else if (c == "rdown")
            wm.RunMouseRightButtonDown(I(1), I(2));
        else if (c == "rup")
            wm.RunMouseRightButtonUp(I(1), I(2));
        else if (c == "rdbl")
            wm.RunMouseRightButtonDoubleClick(I(1), I(2));
        else if (c == "mdown")
            wm.RunMouseMiddleButtonDown(I(1), I(2));
        else if (c == "mup")
            wm.RunMouseMiddleButtonUp(I(1), I(2));
        else if (c == "keydown")
            wm.RunKeyDown(I(1));
        else if (c == "keyup")
            wm.RunKeyUp(I(1));
        else if (c == "esc")
            wm.RunPressEscapeKey();
        else if (c == "exitkey")
            wm.RunPressExitKey();
        else if (c == "imeupdate")
            wm.RunIMEUpdate();
        else if (c == "imetab")
            wm.RunIMETabEvent();
        else if (c == "imereturn")
            wm.RunIMEReturnEvent();
        else if (c == "imekeydown")
            wm.RunIMEKeyDown(I(1));
        else if (c == "imecodepage")
            wm.RunChangeCodePage();
        else if (c == "imeopencand")
            wm.RunOpenCandidate();
        else if (c == "imeclosecand")
            wm.RunCloseCandidate();
        else if (c == "imeopenread")
            wm.RunOpenReading();
        else if (c == "imecloseread")
            wm.RunCloseReading();
        else if (c == "attach")
            wm.AttachIcon(I(1), I(2), I(3), I(4), I(5));
        else if (c == "deattach")
            wm.DeattachIcon();
        else if (c == "attachflag")
            wm.SetAttachingFlag(I(1));
        else if (c == "resetcapture")
            wm.ResetCapture();
        else if (c == "enable" && w)
            static_cast<CButton *>(w)->Enable();
        else if (c == "disable" && w)
            static_cast<CButton *>(w)->Disable();
        else if (c == "btn" && w)
        {
            auto *b = static_cast<CButton *>(w);
            if (a[2] == "up")
                b->Up();
            else if (a[2] == "down")
                b->Down();
            else if (a[2] == "over")
                b->Over();
            else if (a[2] == "setup")
                b->SetUp();
            else if (a[2] == "flash")
                b->Flash();
        }
        else if (c == "restrict" && w)
            static_cast<CDragButton *>(w)->SetRestrictMovementArea(I(2), I(3), I(4), I(5));
        else if (c == "anidelay" && w)
            static_cast<CAniImageBox *>(w)->SetDelay(I(2));
        else if (c == "pickwin" && w)
            std::printf("pickwin %s -> %s\n", NameOf(w).c_str(), NameOf(w->PickWindow(I(2), I(3))).c_str());
        else if (c == "picktop" && w)
            std::printf("picktop %s -> %s\n", NameOf(w).c_str(), NameOf(w->PickTopWindow(I(2), I(3))).c_str());
        else if (c == "mgrpick")
            std::printf("mgrpick -> %s\n", NameOf(wm.__PickWindow(I(1), I(2))).c_str());
        else if (c == "query" && w)
        {
            long lx, ly;
            w->GetMouseLocalPosition(lx, ly);
            std::printf("q %s rendering=%d root=%s parent=%s kids=%d in=%d local=%ld,%ld ischild=%d\n",
                        NameOf(w).c_str(), w->IsRendering(), NameOf(w->GetRoot()).c_str(),
                        NameOf(w->GetParent()).c_str(), w->GetChildCount(), w->IsIn(), lx, ly,
                        a.size() > 2 && W(2) ? w->IsChild(W(2)) : -1);
        }
        else if (c == "dumptree")
            DumpWindow(wm.m_pRootWindow, 0);
        else if (c == "dump" && w)
            DumpWindow(w, 0);
        else if (c == "dumpmgr")
            DumpMgr();
        else if (c == "dumpslot" && w)
            DumpSlot(w);
        // ---- slots
        else if (c == "slot_append" && S())
            S()->AppendSlot(I(2), I(3), I(4), I(5), I(6));
        else if (c == "slot_set" && S())
        {
            D3DXCOLOR col(1, 1, 1, 1);
            S()->SetSlot(I(2), I(3), I(4), I(5), nullptr, col);
        }
        else if (c == "slot_clear" && S())
            S()->ClearSlot((DWORD)I(2));
        else if (c == "slot_clearall" && S())
            S()->ClearAllSlot();
        else if (c == "slot_count" && S())
            S()->SetSlotCount(I(2), I(3));
        else if (c == "slot_countnew" && S())
            S()->SetSlotCountNew(I(2), I(3), I(4));
        else if (c == "slot_cooltime" && S())
            S()->SetSlotCoolTime(I(2), F(3), F(4));
        else if (c == "slot_lock" && S())
            S()->LockSlot(I(2));
        else if (c == "slot_unlock" && S())
            S()->UnlockSlot(I(2));
        else if (c == "slot_cantuse" && S())
            S()->SetCantUseSlot(I(2));
        else if (c == "slot_use" && S())
            S()->SetUseSlot(I(2));
        else if (c == "slot_enable" && S())
            S()->EnableSlot(I(2));
        else if (c == "slot_disable" && S())
            S()->DisableSlot(I(2));
        else if (c == "slot_activate" && S())
            S()->ActivateSlot(I(2));
        else if (c == "slot_deactivate" && S())
            S()->DeactivateSlot(I(2));
        else if (c == "slot_select" && S())
            S()->SelectSlot(I(2));
        else if (c == "slot_clearselected" && S())
            S()->ClearSelected();
        else if (c == "slot_style" && S())
            S()->SetSlotStyle(I(2));
        else if (c == "slot_type" && S())
            S()->SetSlotType(I(2));
        else if (c == "slot_usemode" && S())
            S()->SetUseMode(I(2));
        else if (c == "slot_usable" && S())
            S()->SetUsableItem(I(2));
        else if (c == "slot_refresh" && S())
            S()->RefreshSlot();
        else if (c == "slot_cover" && S())
        {
            S()->SetCoverButton(I(2), "a.sub", "b.sub", "c.sub", "d.sub", I(3), I(4));
            NameInternals();
        }
        else if (c == "slot_coverenable" && S())
            S()->EnableCoverButton(I(2));
        else if (c == "slot_coverdisable" && S())
            S()->DisableCoverButton(I(2));
        else if (c == "slot_alwayscover" && S())
            S()->SetAlwaysRenderCoverButton(I(2), I(3));
        else if (c == "slot_showbase" && S())
            S()->ShowSlotBaseImage(I(2));
        else if (c == "slot_hidebase" && S())
            S()->HideSlotBaseImage(I(2));
        else if (c == "slot_button" && S())
        {
            S()->AppendSlotButton("a.sub", "b.sub", "c.sub");
            NameInternals();
        }
        else if (c == "slot_showbutton" && S())
            S()->ShowSlotButton(I(2));
        else if (c == "slot_hidebuttons" && S())
            S()->HideAllSlotButton();
        else if (c == "slot_sign" && S())
        {
            S()->AppendRequirementSignImage("a.sub");
            NameInternals();
        }
        else if (c == "slot_showsign" && S())
            S()->ShowRequirementSign(I(2));
        else if (c == "slot_hidesign" && S())
            S()->HideRequirementSign(I(2));
        else if (c == "slot_internalsize" && S())
        {
            // Stands in for the image size the buttons would get from their visuals.
            CSlotWindow::TSlot *sl = nullptr;
            if (S()->GetSlotPointer(I(3), &sl))
            {
                CWindow *t = a[2] == "cover" ? (CWindow *)sl->pCoverButton
                                             : a[2] == "btn" ? (CWindow *)sl->pSlotButton : (CWindow *)sl->pSignImage;
                if (t)
                    t->SetSize(I(4), I(5));
            }
        }
        else if (c == "grid" && w)
        {
            static_cast<CGridSlotWindow *>(w)->ArrangeGridSlot(I(2), I(3), I(4), I(5), I(6), I(7), I(8));
            NameInternals();
        }
        // ---- text line
        else if (c == "font" && w)
        {
            EnsureFont();
            static_cast<CTextLine *>(w)->m_TextInstance.SetTextPointer(g_font);
        }
        else if (c == "text" && w)
        {
            std::string s = UnHex(a[2]);
            PrefillText(s);
            static_cast<CTextLine *>(w)->SetText(s.c_str());
        }
        else if (c == "textalign" && w)
        {
            static_cast<CTextLine *>(w)->SetHorizontalAlign(I(2));
            static_cast<CTextLine *>(w)->SetVerticalAlign(I(3));
        }
        else if (c == "limitwidth" && w)
            static_cast<CTextLine *>(w)->SetLimitWidth(F(2));
        else if (c == "secret" && w)
            static_cast<CTextLine *>(w)->SetSecret(I(2));
        else if (c == "multiline" && w)
            static_cast<CTextLine *>(w)->SetMultiLine(I(2));
        else if (c == "fontcolor" && w)
            static_cast<CTextLine *>(w)->SetFontColor((DWORD)std::strtoul(a[2].c_str(), nullptr, 16));
        else if (c == "dumptext" && w)
        {
            auto *tl = static_cast<CTextLine *>(w);
            tl->m_TextInstance.Update();
            std::string label = "text " + NameOf(w);
            DumpTextInstance(label.c_str(), tl->m_TextInstance);
            std::printf("textpos %08x,%08x cursor=%d\n", FBits(tl->m_TextInstance.m_v3Position.x),
                        FBits(tl->m_TextInstance.m_v3Position.y), tl->GetCursorPosition());
        }
        else
            std::printf("!! unknown or inapplicable command\n");
        FlushEvents();
    }
}

// ---- text corpus ------------------------------------------------------------

void RunText(const char *path)
{
    std::ifstream in(path);
    std::string line;
    int n = 0;
    EnsureFont();
    while (std::getline(in, line))
    {
        std::istringstream ss(line);
        UINT cp;
        std::string hex;
        ss >> cp >> hex;
        std::string bytes = UnHex(hex);
        SetDefaultCodePage(cp);
        std::vector<wchar_t> wbuf(bytes.size() * 2 + 4);
        int wl = Ymir_MultiByteToWideChar(cp, 0, bytes.data(), bytes.size(), wbuf.data(), wbuf.size());
        if (wl < 0)
            wl = 0;
        std::wstring w(wbuf.data(), wl);
        std::printf("t %d cp=%u\n", n++, cp);
        std::printf(" w %s\n", WHex(w).c_str());
        std::printf(" out %s olen=%d\n", WHex(GetTextTagOutputString(w.c_str(), wl)).c_str(),
                    GetTextTagOutputLen(w.c_str(), wl));
        std::printf(" tags");
        std::wstring extra;
        for (int i = 0; i < wl;)
        {
            int len = 0;
            int tag = GetTextTag(&w[i], wl - i, len, extra);
            std::printf(" %d:%d:%d:%s", i, tag, len, tag == TEXT_TAG_COLOR ? WHex(extra).c_str() : "-");
            i += len;
        }
        std::printf("\n ipos");
        int olen = GetTextTagOutputLen(w.c_str(), wl);
        for (int o = 0; o <= olen + 1; ++o)
            std::printf(" %d", GetTextTagInternalPosFromRenderPos(w.c_str(), wl, o));
        std::printf("\n fe");
        for (int p = 0; p <= wl; ++p)
            std::printf(" %d", FindColorTagEndPosition(w.c_str() + p, wl - p));
        std::printf("\n fs");
        for (int p = 1; p <= wl; ++p)
            std::printf(" %d", FindColorTagStartPosition(w.c_str() + p - 1, p));
        std::printf("\n");
        // layout with the fake font, two limit widths and secret mode
        PrefillText(bytes);
        for (int mode = 0; mode < 3; ++mode)
        {
            CGraphicTextInstance ti;
            ti.SetTextPointer(g_font);
            ti.SetColor((DWORD)0xff112233);
            ti.SetLimitWidth(mode == 1 ? 60.0f : mode == 2 ? 25.0f : 1600.0f);
            ti.SetSecret(mode == 2);
            ti.SetValue(bytes.c_str());
            ti.Update();
            const char *lab = mode == 0 ? " L1600" : mode == 1 ? " L60" : " S25";
            DumpTextInstance(lab, ti);
            ti.m_pCharInfoVector.clear();
            ti.m_roText = nullptr;
        }
    }
}

const char *kPySetup = R"PY(
_out = []
_rets = {}
class _H(object):
    def __init__(self, wid):
        self._wid = wid
    def __getattr__(self, name):
        if name.startswith('__') or name.startswith('_'):
            raise AttributeError(name)
        key = (self._wid, name)
        if key not in _rets:
            raise AttributeError(name)
        r = _rets[key]
        wid = self._wid
        def f(*a):
            _out.append("ev %d %s %s\n" % (wid, name, ",".join("%d" % x for x in a)))
            return r
        return f
def _define(wid, name, ret):
    _rets[(wid, name)] = None if ret == 'none' else int(ret)
def _flush():
    s = "".join(_out)
    del _out[:]
    return s
)PY";

} // namespace

int main(int argc, char **argv)
{
    if (argc < 3)
    {
        std::fprintf(stderr, "usage: m2diff_ui scenario|text <file>\n");
        return 2;
    }
    setvbuf(stdout, nullptr, _IOFBF, 1 << 16);
    CTimer timer;
    timer.UseCustomTime();
    CEterPackManager packs;
    CPythonLauncher launcher;
    launcher.Create("m2diff_ui");
    CPythonGraphic graphic;
    CPythonResource resource;
    CGraphicImageInstance::CreateSystem(256);
    CGraphicExpandedImageInstance::CreateSystem(256);
    CGraphicMarkInstance::CreateSystem(16);
    CGraphicTextInstance::CreateSystem(16);
    CWindowManager wm;
    SetDefaultCodePage(65001);

    PyRun_SimpleString(kPySetup);
    PyObject *main = PyImport_AddModule("__main__");
    PyObject *dict = PyModule_GetDict(main);
    g_hClass = PyDict_GetItemString(dict, "_H");
    g_flush = PyDict_GetItemString(dict, "_flush");
    g_define = PyDict_GetItemString(dict, "_define");

    if (!std::strcmp(argv[1], "scenario"))
    {
        g_names[wm.m_pRootWindow] = "ROOT";
        for (auto &kv : wm.m_LayerWindowMap)
            g_names[kv.second] = "L:" + kv.first;
        RunScenario(argv[2]);
    }
    else
        RunText(argv[2]);
    std::fflush(stdout);
    _exit(0); // skip destructors of the half-faked font and the singletons
}
