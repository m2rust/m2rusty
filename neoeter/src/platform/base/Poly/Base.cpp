#include <Neoeter/Platform/Base/StdAfx.h>
#include <Neoeter/Platform/Base/Poly/Base.h>

CBase::CBase() : id(0)
{
}

CBase::~CBase() = default;

bool CBase::isNumber() const
{
    return (id & MID_NUMBER) != 0;
}

bool CBase::isVar() const
{
    return (id & MID_VARIABLE) != 0;
}

bool CBase::isSymbol() const
{
    return (id & MID_SYMBOL) != 0;
}
