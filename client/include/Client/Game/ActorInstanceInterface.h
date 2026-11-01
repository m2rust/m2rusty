#pragma once

#include <Neoeter/Model/Grn/ThingInstance.h>

class IActorInstance : public CGraphicThingInstance
{
  public:
    enum
    {
        ID = ACTOR_OBJECT
    };
    int GetType() const override
    {
        return ID;
    }

    IActorInstance() = default;
    ~IActorInstance() override = default;
    virtual bool TestCollisionWithDynamicSphere(const CDynamicSphereInstance &dsi) = 0;
    virtual DWORD GetVirtualID() = 0;
};