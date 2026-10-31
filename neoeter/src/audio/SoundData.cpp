#include <Neoeter/Audio/StdAfx.h>
#include <Neoeter/Audio/SoundData.h>

#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/Timer.h>

bool CSoundData::ms_isSoundFile[SOUND_FILE_MAX_NUM];
std::stringstream CSoundData::ms_SoundFile[SOUND_FILE_MAX_NUM];

const char * CSoundData::GetFileName()
{
	return m_filename;
}

void CSoundData::Assign(const char* filename)
{
	assert(m_assigned == false);

	strncpy(m_filename, filename, sizeof(m_filename)-1);
	m_filename[sizeof(m_filename)-1] = '\0';
	m_assigned = true;
}

LPVOID CSoundData::Get()
{
	++m_iRefCount;
	m_dwAccessTime = ELTimer_GetMSec(); 

	if (!m_data)
		ReadFromDisk();

	if (m_flag & FLAG_DATA_SIZE)
		return ((S32 *) m_data + 1);
	else
		return (m_data);
}
 
ULONG CSoundData::GetSize()
{
	return m_size;
}

void CSoundData::Release()
{
	assert(m_iRefCount != 0);
	--m_iRefCount;
	m_dwAccessTime = ELTimer_GetMSec();
}

DWORD CSoundData::GetPlayTime()
{
	return m_dwPlayTime;
}

void CSoundData::SetPlayTime(DWORD dwPlayTime)
{
	m_dwPlayTime = dwPlayTime;
}

DWORD CSoundData::GetAccessTime()
{
	return m_dwAccessTime;
}

bool CSoundData::ReadFromDisk()
{
	assert(m_assigned == true);

	LARGE_INTEGER start;
	QueryPerformanceCounter(&start);

	U32* s = (U32 *) AIL_file_read(m_filename, FILE_READ_WITH_SIZE);

	if (s == NULL)
		return false;

	S32 type = AIL_file_type(s + 1, s[0]);
	AILSOUNDINFO info;
	
	switch (type)
	{
		case AILFILETYPE_PCM_WAV:
			{
				m_data = s;
				m_size = *((S32 *) s);
				m_flag |= FLAG_DATA_SIZE;
			}
			break;

		case AILFILETYPE_ADPCM_WAV:	// 3D 사운드는 decompress 해야 함.
			{
				AIL_WAV_info(s + 1, &info);
				AIL_decompress_ADPCM(&info, &m_data, (U32*)&m_size);  // ULONG == U32 width on LLP64
				AIL_mem_free_lock(s);
			}
			break;

		case AILFILETYPE_MPEG_L3_AUDIO:
			{
				AIL_decompress_ASI(s + 1, *((S32 *) s), m_filename, &m_data, (U32*)&m_size, 0);  // LLP64: ULONG vs U32
				AIL_mem_free_lock(s);
			}
			break;

		default:
			assert(!"Unknown File Type");
			AIL_mem_free_lock(s);
			return false;
	}

	return true;
}

bool CSoundData::isSlotIndex(DWORD dwIndex)
{
	if (dwIndex >= SOUND_FILE_MAX_NUM)
		return false;

	if (!ms_isSoundFile[dwIndex])
		return false;

	return true;
}

int CSoundData::GetEmptySlotIndex()
{
	for (int i = 0; i < SOUND_FILE_MAX_NUM; ++i)
	{
		if (!ms_isSoundFile[i])
			return i;
	}

	return -1;
}

U32 AILCALLBACK CSoundData::open_callback(char const * filename, U32 *file_handle)
{
	int iIndex = GetEmptySlotIndex();

	if (-1 == iIndex)
		return 0;

	if (!CEterPackManager::Instance().Get(filename, ms_SoundFile[iIndex]))
		return 0;

	ms_isSoundFile[iIndex] = true;
	
	*file_handle = iIndex;
	return 1;
}

void AILCALLBACK CSoundData::close_callback(U32 file_handle)
{
	if (!isSlotIndex(file_handle))
		return;

	ms_SoundFile[file_handle].str("");
	ms_isSoundFile[file_handle] = false;
}

S32 AILCALLBACK CSoundData::seek_callback(U32 file_handle, S32 offset, U32 type)
{
	if (!isSlotIndex(file_handle))
		return 0;

	ms_SoundFile[file_handle].seekg(offset, static_cast<std::ios_base::seekdir>(type));
	return static_cast<S32>(ms_SoundFile[file_handle].tellg());
}

U32 AILCALLBACK CSoundData::read_callback(U32 file_handle, void * buffer, U32 bytes)
{
	if (!isSlotIndex(file_handle))
		return 0;

	ms_SoundFile[file_handle].read((char*) buffer, bytes);
	return static_cast<U32>(ms_SoundFile[file_handle].gcount());
}

void CSoundData::SetPackMode()
{
	// if sound data reads from pack file, the callbacks of the MSS should be changed.
	AIL_set_file_callbacks(open_callback, close_callback, seek_callback, read_callback);

	for (int i = 0; i < SOUND_FILE_MAX_NUM; ++i)
		ms_isSoundFile[i] = false;
}

void CSoundData::Destroy()
{
	if (m_data)
	{
		AIL_mem_free_lock(m_data);
		m_data = NULL;
	}
}

CSoundData::CSoundData() : 
m_iRefCount(0),
m_dwAccessTime(ELTimer_GetMSec()),
m_dwPlayTime(0),
m_size(0),
m_data(NULL),
m_flag(0),
m_assigned(false)
{
	m_filename[0] = '\0';
}

CSoundData::~CSoundData()
{
	Destroy();
}
