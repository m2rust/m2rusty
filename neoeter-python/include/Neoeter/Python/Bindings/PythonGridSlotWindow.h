#pragma once

#include <Neoeter/Python/Bindings/PythonSlotWindow.h>

namespace UI
{
class CGridSlotWindow : public CSlotWindow
{
  public:
    static DWORD Type();

    CGridSlotWindow(PyObject *ppyObject);
    ~CGridSlotWindow() override;

    void Destroy();

    void ArrangeGridSlot(DWORD dwStartIndex, DWORD dwxCount, DWORD dwyCount, int ixSlotSize, int iySlotSize,
                         int ixTemporarySize, int iyTemporarySize);

  protected:
    void __Initialize();

    BOOL GetPickedSlotPointer(TSlot **ppSlot) override;
    BOOL GetPickedSlotList(int iWidth, int iHeight, std::list<TSlot *> *pSlotPointerList);
    BOOL GetGridSlotPointer(int ix, int iy, TSlot **ppSlot);
    BOOL GetPickedGridSlotPosition(int ixLocal, int iyLocal, int *pix, int *piy);
    BOOL CheckMoving(DWORD dwSlotNumber, DWORD dwItemIndex, const std::list<TSlot *> &c_rSlotList);

    BOOL OnIsType(DWORD dwType) override;

    void OnRefreshSlot() override;
    void OnRenderPickingSlot() override;

    DWORD m_dwxCount = 0U;
    DWORD m_dwyCount = 0U;

    std::vector<TSlot *> m_SlotVector;
};
}; // namespace UI
