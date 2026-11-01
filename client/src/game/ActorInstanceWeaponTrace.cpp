#include <Client/Game/StdAfx.h>
#include <Client/Game/ActorInstance.h>
#include <Client/Game/WeaponTrace.h>

#include <algorithm>

void CActorInstance::TraceProcess()
{
    if (!m_WeaponTraceVector.empty())
    {
        std::vector<CWeaponTrace *>::iterator it;
        for (it = m_WeaponTraceVector.begin(); it != m_WeaponTraceVector.end(); ++it)
        {
            CWeaponTrace *pWeaponTrace = (*it);
            pWeaponTrace->SetPosition(m_x, m_y, m_z);
            pWeaponTrace->SetRotation(m_fcurRotation);
            pWeaponTrace->Update(__GetReachScale());
        }
    }
}

void CActorInstance::RenderTrace()
{
    std::ranges::for_each(m_WeaponTraceVector, std::void_mem_fun(&CWeaponTrace::Render));
}

void CActorInstance::__DestroyWeaponTrace()
{
    std::ranges::for_each(m_WeaponTraceVector, CWeaponTrace::Delete);
    m_WeaponTraceVector.clear();
}

void CActorInstance::__ShowWeaponTrace()
{
    std::ranges::for_each(m_WeaponTraceVector, std::void_mem_fun(&CWeaponTrace::TurnOn));
}

void CActorInstance::__HideWeaponTrace()
{
    std::ranges::for_each(m_WeaponTraceVector, std::void_mem_fun(&CWeaponTrace::TurnOff));
}
