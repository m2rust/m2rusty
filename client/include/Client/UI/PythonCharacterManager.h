#pragma once

#include <Client/UI/AbstractCharacterManager.h>
#include <Client/UI/InstanceBase.h>
#include <Client/Game/PhysicsObject.h>

class CPythonCharacterManager : public CSingleton<CPythonCharacterManager>,
                                public IAbstractCharacterManager,
                                public IObjectManager
{
  public:
    // Character List
    using TCharacterInstanceList = std::list<CInstanceBase *>;
    using TCharacterInstanceMap = std::map<DWORD, CInstanceBase *>;

    class CharacterIterator;

    CPythonCharacterManager();
    ~CPythonCharacterManager() override;

    void AdjustCollisionWithOtherObjects(CActorInstance *pInst) override;

    void EnableSortRendering(bool isEnable);

    bool IsRegisteredVID(DWORD dwVID);
    bool IsAliveVID(DWORD dwVID);
    bool IsDeadVID(DWORD dwVID);
    bool IsCacheMode();

    bool OLD_GetPickedInstanceVID(DWORD *pdwPickedActorID);
    CInstanceBase *OLD_GetPickedInstancePtr();
    D3DXVECTOR2 &OLD_GetPickedInstPosReference();

    CInstanceBase *FindClickableInstancePtr();

    void InsertPVPKey(DWORD dwVIDSrc, DWORD dwVIDDst);
    void RemovePVPKey(DWORD dwVIDSrc, DWORD dwVIDDst);
    void ChangeGVG(DWORD dwSrcGuildID, DWORD dwDstGuildID);

    void GetInfo(std::string *pstInfo);

    void ClearMainInstance();
    bool SetMainInstance(DWORD dwVID);
    CInstanceBase *GetMainInstancePtr();

    void SCRIPT_SetAffect(DWORD dwVID, DWORD eState, BOOL isVisible);
    void SetEmoticon(DWORD dwVID, DWORD eState);
    bool IsPossibleEmoticon(DWORD dwVID);
    void ShowPointEffect(DWORD ePoint, DWORD dwVID);
    bool RegisterPointEffect(DWORD ePoint, const char *c_szFileName);

    // System
    void Destroy() override;

    void DeleteAllInstances();

    bool CreateDeviceObjects();
    void DestroyDeviceObjects();

    void Update();
    void Deform();
    void Render();
    void RenderShadowMainInstance();
    void RenderShadowAllInstances();
    void RenderCollision();

    // Create/Delete Instance
    CInstanceBase *CreateInstance(const CInstanceBase::SCreateData &c_rkCreateData);
    CInstanceBase *RegisterInstance(DWORD VirtualID);

    void DeleteInstance(DWORD dwDelVID);
    void DeleteInstanceByFade(DWORD dwVID);
    void DeleteVehicleInstance(DWORD VirtualID);

    void DestroyAliveInstanceMap();
    void DestroyDeadInstanceList();

    CharacterIterator CharacterInstanceBegin()
    {
        return CharacterIterator(m_kAliveInstMap.begin());
    }
    CharacterIterator CharacterInstanceEnd()
    {
        return CharacterIterator(m_kAliveInstMap.end());
    }

    // Access Instance
    void SelectInstance(DWORD VirtualID);
    CInstanceBase *GetSelectedInstancePtr();

    CInstanceBase *GetInstancePtr(DWORD VirtualID) override;
    CInstanceBase *GetInstancePtrByName(const char *name);

    // Pick
    int PickAll();
    CInstanceBase *GetCloseInstance(CInstanceBase *pInstance);

    // Refresh TextTail
    void RefreshAllPCTextTail();
    void RefreshAllGuildMark();

  protected:
    void UpdateTransform();
    void UpdateDeleting();

    void __Initialize();

    void __DeleteBlendOutInstance(CInstanceBase *pkInstDel);

    void __OLD_Pick();
    void __NEW_Pick();

    void __UpdateSortPickedActorList();
    void __UpdatePickedActorList();
    void __SortPickedActorList();

    void __RenderSortedAliveActorList();
    void __RenderSortedDeadActorList();

    CInstanceBase *m_pkInstMain = nullptr;
    CInstanceBase *m_pkInstPick = nullptr;
    CInstanceBase *m_pkInstBind = nullptr;
    D3DXVECTOR2 m_v2PickedInstProjPos;

    TCharacterInstanceMap m_kAliveInstMap;
    TCharacterInstanceList m_kDeadInstList;

    std::vector<CInstanceBase *> m_kVct_pkInstPicked;

    DWORD m_adwPointEffect[POINT_MAX_NUM]{};

  public:
    class CharacterIterator
    {
      public:
        CharacterIterator() = default;
        CharacterIterator(const TCharacterInstanceMap::iterator &it) : m_it(it)
        {
        }

        CInstanceBase *operator*()
        {
            return m_it->second;
        }

        CharacterIterator &operator++()
        {
            ++m_it;
            return *this;
        }

        CharacterIterator operator++(int)
        {
            CharacterIterator const new_it = *this;
            ++(*this);
            return new_it;
        }

        CharacterIterator &operator=(const CharacterIterator &rhs) = default;

        bool operator==(const CharacterIterator &rhs) const
        {
            return m_it == rhs.m_it;
        }

        bool operator!=(const CharacterIterator &rhs) const
        {
            return m_it != rhs.m_it;
        }

      private:
        TCharacterInstanceMap::iterator m_it;
    };
};
