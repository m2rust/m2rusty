#include <Neoeter/Platform/Base/StdAfx.h>
#include <Neoeter/Platform/Base/Poly/SymTable.h>

#include <utility>

using namespace std;

CSymTable::CSymTable(int aTok, string aStr) : token(aTok), strlex(std::move(aStr))
{
}

CSymTable::~CSymTable() = default;
