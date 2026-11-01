#pragma once

#include "StdAfx.h"

BOOL ShiftJIS_IsLeadByte(char chByte);
BOOL ShiftJIS_IsTrailByte(char chByte);
int ShiftJIS_StringCompareCI(LPCSTR szStringLeft, LPCSTR szStringRight, size_t sizeLength);
