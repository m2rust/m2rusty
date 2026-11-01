#pragma once

#include <Client/UI/AbstractSingleton.h>

class CInstanceBase;

class IAbstractCharacterManager : public TAbstractSingleton<IAbstractCharacterManager>
{
  public:
    IAbstractCharacterManager() = default;
    ~IAbstractCharacterManager() override = default;

    virtual void Destroy() = 0;
    virtual CInstanceBase *GetInstancePtr(DWORD dwVID) = 0;
};