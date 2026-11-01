#pragma once

#include <Neoeter/Platform/Base/Singleton.h>
#include <Neoeter/Render/IME.h>

class CPythonIME : public IIMEEventSink, public CIME, public CSingleton<CPythonIME>
{
  public:
    CPythonIME();
    ~CPythonIME() override;

    void MoveLeft();
    void MoveRight();
    void MoveHome();
    void MoveEnd();
    void SetCursorPosition(int iPosition);
    void Delete();

    void Create(HWND hWnd);

  protected:
    virtual void OnTab();
    virtual void OnReturn();
    virtual void OnEscape();

    bool OnWM_CHAR(WPARAM wParam, LPARAM lParam) override;
    void OnUpdate() override;
    void OnChangeCodePage() override;
    void OnOpenCandidateList() override;
    void OnCloseCandidateList() override;
    void OnOpenReadingWnd() override;
    void OnCloseReadingWnd() override;
};
