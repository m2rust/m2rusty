#pragma once

#include <Client/Game/ItemData.h>

class CItemManager : public CSingleton<CItemManager>
{
  public:
    enum EItemDescCol
    {
        ITEMDESC_COL_VNUM,
        ITEMDESC_COL_NAME,
        ITEMDESC_COL_DESC,
        ITEMDESC_COL_SUMM,
        ITEMDESC_COL_NUM,
    };

    using TItemMap = std::map<DWORD, CItemData *>;
    using TItemNameMap = std::map<std::string, CItemData *>;

    CItemManager();
    ~CItemManager() override;

    void Destroy();

    BOOL SelectItemData(DWORD dwIndex);
    CItemData *GetSelectedItemDataPointer();

    BOOL GetItemDataPointer(DWORD dwItemID, CItemData **ppItemData);

    /////
    bool LoadItemDesc(const char *c_szFileName);
    bool LoadItemList(const char *c_szFileName);
    bool LoadItemTable(const char *c_szFileName);
    CItemData *MakeItemData(DWORD dwIndex);

  protected:
    TItemMap m_ItemMap;
    std::vector<CItemData *> m_vec_ItemRange;
    CItemData *m_pSelectedItemData;
};
