#pragma once

using CRCPair = std::pair<DWORD, std::string>;

void ProcessScanner_Destroy();
bool ProcessScanner_Create();
void ProcessScanner_ReleaseQuitEvent();

bool ProcessScanner_PopProcessQueue(std::vector<CRCPair> *pkVct_crcPair);