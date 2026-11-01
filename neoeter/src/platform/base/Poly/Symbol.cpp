#include <Neoeter/Platform/Base/StdAfx.h>
#include <Neoeter/Platform/Base/Poly/Symbol.h>

CSymbol::CSymbol() : iType(ST_UNKNOWN)
{
    id = MID_SYMBOL;
}

CSymbol::~CSymbol() = default;

bool CSymbol::Equal(const CSymbol &dif) const
{
    return dif.iType / 10 == iType / 10;
}

bool CSymbol::Less(const CSymbol &dif) const
{
    return dif.iType / 10 > iType / 10;
}

int CSymbol::GetType() const
{
    return iType;
}

void CSymbol::SetType(int Type)
{
    iType = Type;
}

int CSymbol::issymbol(int ch)
{
    switch (ch)
    {
    case SY_PLUS:
        return ST_PLUS;
    case SY_MINUS:
        return ST_MINUS;
    case SY_MULTIPLY:
        return ST_MULTIPLY;
    case SY_DIVIDE:
        return SY_DIVIDE;
    case SY_CARET:
        return SY_CARET;
    case SY_OPEN:
        return SY_OPEN;
    case SY_CLOSE:
        return ST_CLOSE;
    }
    return 0;
}
