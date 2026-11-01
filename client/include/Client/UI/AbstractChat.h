#pragma once

#include <Client/UI/AbstractSingleton.h>

class IAbstractChat : public TAbstractSingleton<IAbstractChat>
{
  public:
    IAbstractChat() = default;
    ~IAbstractChat() override = default;

    virtual void AppendChat(int iType, const char *c_szChat) = 0;
};