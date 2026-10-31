#include <Client/UI/StdAfx.h>
#include <map>
#include <string>

struct ApplicationStringTable 
{
	HINSTANCE m_hInstance;
	std::map<DWORD, std::string> m_kMap_dwID_stLocale;
} gs_kAppStrTable;

void ApplicationStringTable_Initialize(HINSTANCE hInstance)
{
	gs_kAppStrTable.m_hInstance=hInstance;
}

const std::string& ApplicationStringTable_GetString(DWORD dwID, LPCSTR szKey)
{
	char szBuffer[512];
	char szIniFileName[256];
	char szLocale[256];

	::GetCurrentDirectory(sizeof(szIniFileName), szIniFileName);
	if(szIniFileName[lstrlen(szIniFileName)-1] != '\\' && szIniFileName[lstrlen(szIniFileName)-1] != '/')
		strcat(szIniFileName, "/");
	strcat(szIniFileName, "metin2client.dat");

	strcpy(szLocale, LocaleService_GetLocalePath());
	if(strnicmp(szLocale, "locale/", strlen("locale/")) == 0)
		strcpy(szLocale, LocaleService_GetLocalePath() + strlen("locale/"));
	::GetPrivateProfileString(szLocale, szKey, NULL, szBuffer, sizeof(szBuffer)-1, szIniFileName);
	if(szBuffer[0] == '\0')
		LoadString(gs_kAppStrTable.m_hInstance, dwID, szBuffer, sizeof(szBuffer)-1);
	if(szBuffer[0] == '\0')
		::GetPrivateProfileString("en", szKey, NULL, szBuffer, sizeof(szBuffer)-1, szIniFileName);
	if(szBuffer[0] == '\0')
		strcpy(szBuffer, szKey);

	std::string& rstLocale=gs_kAppStrTable.m_kMap_dwID_stLocale[dwID];
	rstLocale=szBuffer;

	return rstLocale;
}

const std::string& ApplicationStringTable_GetString(DWORD dwID)
{
	char szBuffer[512];

	LoadString(gs_kAppStrTable.m_hInstance, dwID, szBuffer, sizeof(szBuffer)-1);
	std::string& rstLocale=gs_kAppStrTable.m_kMap_dwID_stLocale[dwID];
	rstLocale=szBuffer;

	return rstLocale;
}

const char* ApplicationStringTable_GetStringz(DWORD dwID, LPCSTR szKey)
{
	return ApplicationStringTable_GetString(dwID, szKey).c_str();
}

const char* ApplicationStringTable_GetStringz(DWORD dwID)
{
	return ApplicationStringTable_GetString(dwID).c_str();
}
