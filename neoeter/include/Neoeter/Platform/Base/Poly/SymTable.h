#ifndef __POLY_SYMTABLE_H__
#define __POLY_SYMTABLE_H__

#include <string>

class CSymTable
{
  public:
    CSymTable(int aTok, std::string aStr);
    virtual ~CSymTable();

    double dVal = 0;
    int token;
    std::string strlex;
};

#endif
