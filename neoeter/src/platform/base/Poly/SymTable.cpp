#include <Neoeter/Platform/Base/StdAfx.h>
#include <Neoeter/Platform/Base/Poly/SymTable.h>

using namespace std;

CSymTable::CSymTable(int aTok, string aStr) : dVal(0), token(aTok), strlex(aStr)
{
}

CSymTable::~CSymTable()
{
}

