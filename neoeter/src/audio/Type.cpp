#include <Neoeter/Audio/StdAfx.h>
#include <Neoeter/Audio/Type.h>
#include <Neoeter/Render/TextFileLoader.h>

std::string NSound::strResult;

const char * NSound::GetResultString()
{
	return strResult.c_str();
}

void NSound::SetResultString(const char * c_pszStr)
{
	strResult.assign(c_pszStr);
}

bool NSound::LoadSoundInformationPiece(const char * c_szFileName, NSound::TSoundDataVector & rSoundDataVector, const char * c_szPathHeader)
{
	std::string strResult;
	strResult = c_szFileName;

	CTextFileLoader* pkTextFileLoader=CTextFileLoader::Cache(c_szFileName);
	if (!pkTextFileLoader)
		return false;

	CTextFileLoader& rkTextFileLoader=*pkTextFileLoader;
	if (rkTextFileLoader.IsEmpty())
	{
		SetResultString((strResult + " \xC0\xD0\xB1\xE2\xBF\xEB \xC6\xC4\xC0\xCF\xC0\xBB \xBF\xAD \xBC\xF6 \xBE\xF8\xC0\xBD").c_str());
		return false;
	}

	rkTextFileLoader.SetTop();

	int iCount;
	if (!rkTextFileLoader.GetTokenInteger("sounddatacount", &iCount))
	{
		SetResultString((strResult + " \xC6\xC4\xC0\xCF \xC6\xF7\xB8\xE4 \xBF\xA1\xB7\xAF, SoundDataCount\xB8\xA6 \xC3\xA3\xC0\xBB \xBC\xF6 \xBE\xF8\xC0\xBD").c_str());
		return false;
	}

	rSoundDataVector.clear();
	rSoundDataVector.resize(iCount);

	char szSoundDataHeader[32+1];
	for (DWORD i = 0; i < rSoundDataVector.size(); ++i)
	{
		_snprintf(szSoundDataHeader, sizeof(szSoundDataHeader), "sounddata%02d", i);
		CTokenVector * pTokenVector;
		if (!rkTextFileLoader.GetTokenVector(szSoundDataHeader, &pTokenVector))
		{
			SetResultString((strResult + " \xC6\xC4\xC0\xCF \xC6\xF7\xB8\xE4 \xBF\xA1\xB7\xAF: " + szSoundDataHeader + " \xB8\xA6 \xC3\xA3\xC0\xBB \xBC\xF6 \xBE\xF8\xC0\xBD").c_str());
			return false;
		}

		if (2 != pTokenVector->size())
		{
			SetResultString((strResult + " \xC6\xC4\xC0\xCF \xC6\xF7\xB8\xE4 \xBF\xA1\xB7\xAF: \xBA\xA4\xC5\xCD \xC5\xA9\xB1\xE2\xB0\xA1 2\xB0\xA1 \xBE\xC6\xB4\xD4").c_str());
			return false;
		}

		rSoundDataVector[i].fTime = (float) atof(pTokenVector->at(0).c_str());
		if (c_szPathHeader)
		{
			rSoundDataVector[i].strSoundFileName = c_szPathHeader;
			rSoundDataVector[i].strSoundFileName += pTokenVector->at(1).c_str();
		}
		else
		{
			rSoundDataVector[i].strSoundFileName = pTokenVector->at(1).c_str();
		}
	}

	SetResultString((strResult + " \xBA\xD2\xB7\xAF\xBF\xC8").c_str());
	return true;
}

bool NSound::SaveSoundInformationPiece(const char * c_szFileName, NSound::TSoundDataVector & rSoundDataVector)
{
	if (rSoundDataVector.empty())	// 데이터가 없으면 성공으로 간주
	{
		if (IsFile(c_szFileName))	// 데이터는 비어있는데 파일이 있다면
		{
#ifdef _WIN32
			_unlink(c_szFileName);		// 지운다.
#else
			unlink(c_szFileName);
#endif
		}
		return true;
	}

	std::string strResult;
	strResult = c_szFileName;

	FILE * File = fopen(c_szFileName, "wt");

	if (!File)
	{
		char szErrorText[256+1];
		_snprintf(szErrorText, sizeof(szErrorText), "Failed to save file (%s).\nPlease check if it is read-only or you have no space on the disk.\n", c_szFileName);
		LogBox(szErrorText, "Errors");
		SetResultString((strResult + " \xBE\xB2\xB1\xE2\xBF\xEB \xC6\xC4\xC0\xCF\xC0\xBB \xBF\xAD \xBC\xF6 \xBE\xF8\xC0\xBD").c_str());
		return false;
	}

	fprintf(File, "ScriptType        CharacterSoundInformation\n");
	fprintf(File, "\n");

	fprintf(File, "SoundDataCount    %d\n", (int)rSoundDataVector.size());

	for (DWORD i = 0; i < rSoundDataVector.size(); ++i)
	{
		NSound::TSoundData & rSoundData = rSoundDataVector[i];
		fprintf(File, "SoundData%02u       %f \"%s\"\n", (unsigned int)i, rSoundData.fTime, rSoundData.strSoundFileName.c_str());
	}

	fclose(File);
	return true;
}

void NSound::DataToInstance(const TSoundDataVector & c_rSoundDataVector, TSoundInstanceVector * pSoundInstanceVector)
{
	if (c_rSoundDataVector.empty())
		return;

	DWORD dwFPS = 60;
	const float c_fFrameTime = 1.0f / float(dwFPS);

	pSoundInstanceVector->clear();
	pSoundInstanceVector->resize(c_rSoundDataVector.size());
	for (DWORD i = 0; i < c_rSoundDataVector.size(); ++i)
	{
		const TSoundData & c_rSoundData = c_rSoundDataVector[i];
		TSoundInstance & rSoundInstance = pSoundInstanceVector->at(i);

		rSoundInstance.dwFrame = (DWORD) (c_rSoundData.fTime / c_fFrameTime);
		rSoundInstance.strSoundFileName = c_rSoundData.strSoundFileName;
	}
}
