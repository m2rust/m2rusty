#pragma once

#include <Client/Game/ActorInstance.h>
#include <Client/Game/FlyHandler.h>

#include <Client/UI/PythonNetworkStream.h>
#include <Client/UI/InstanceBase.h>

class CPythonPlayerEventHandler : public CActorInstance::IEventHandler
{
  public:
    static CPythonPlayerEventHandler &GetSingleton();

    ~CPythonPlayerEventHandler() override;

    void OnSyncing(const SState &c_rkState) override;
    void OnWaiting(const SState &c_rkState) override;
    void OnMoving(const SState &c_rkState) override;
    void OnMove(const SState &c_rkState) override;
    void OnStop(const SState &c_rkState) override;
    void OnWarp(const SState &c_rkState) override;
    void OnClearAffects() override;
    void OnSetAffect(UINT uAffect) override;
    void OnResetAffect(UINT uAffect) override;
    void OnAttack(const SState &c_rkState, WORD wMotionIndex) override;
    void OnUseSkill(const SState &c_rkState, UINT uMotSkill, UINT uArg) override;
    virtual void OnUpdate();
    void OnChangeShape() override;
    void OnHit(UINT uSkill, CActorInstance &rkActorVictim, BOOL isSendPacket) override;

    void FlushVictimList();

  protected:
    CPythonPlayerEventHandler();

    struct SVictim
    {
        DWORD m_dwVID;
        long m_lPixelX;
        long m_lPixelY;
    };

    std::vector<SVictim> m_kVctkVictim;

    DWORD m_dwPrevComboIndex;
    DWORD m_dwNextWaitingNotifyTime;
    DWORD m_dwNextMovingNotifyTime;
    TPixelPosition m_kPPosPrevWaiting;

  private:
    class CNormalBowAttack_FlyEventHandler_AutoClear : public IFlyEventHandler
    {
      public:
        CNormalBowAttack_FlyEventHandler_AutoClear() = default;
        ~CNormalBowAttack_FlyEventHandler_AutoClear() override = default;

        void Set(CPythonPlayerEventHandler *pParent, CInstanceBase *pInstMain, CInstanceBase *pInstTarget);
        void SetTarget(CInstanceBase *pInstTarget);

        void OnSetFlyTarget() override;
        void OnShoot(DWORD dwSkillIndex) override;

        void OnNoTarget() override
        { /*Tracenf("Shoot : target이 없습니다.");*/
        }
        void OnExplodingOutOfRange() override
        { /*Tracenf("Shoot : 사정거리가 끝났습니다.");*/
        }
        void OnExplodingAtBackground() override
        { /*Tracenf("Shoot : 배경에 맞았습니다.");*/
        }
        void OnExplodingAtAnotherTarget(DWORD dwSkillIndex, DWORD dwVID) override;
        void OnExplodingAtTarget(DWORD dwSkillIndex) override;

      protected:
        CPythonPlayerEventHandler *m_pParent = nullptr;
        CInstanceBase *m_pInstMain = nullptr;
        CInstanceBase *m_pInstTarget = nullptr;
    } m_NormalBowAttack_FlyEventHandler_AutoClear;

  public:
    IFlyEventHandler *GetNormalBowAttackFlyEventHandler(CInstanceBase *pInstMain, CInstanceBase *pInstTarget)
    {
        m_NormalBowAttack_FlyEventHandler_AutoClear.Set(this, pInstMain, pInstTarget);
        return &m_NormalBowAttack_FlyEventHandler_AutoClear;
    }

    void ChangeFlyTarget(CInstanceBase *pInstTarget)
    {
        m_NormalBowAttack_FlyEventHandler_AutoClear.SetTarget(pInstTarget);
    }
};
