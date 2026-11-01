#ifndef __MILESLIB_CSOUNDDATA_H__
#define __MILESLIB_CSOUNDDATA_H__

#include <sstream>
// T4.1: Miles is gone on every target; MSSCompat is the miniaudio shim.
#include "MSSCompat.h"
#include <Neoeter/Platform/Base/MappedFile.h>

class CSoundData
{
  public:
    enum
    {
        FLAG_DATA_SIZE = 1,
        SOUND_FILE_MAX_NUM = 5,
    };

    static void SetPackMode();

    CSoundData();
    virtual ~CSoundData();

    void Assign(const char *filename);
    LPVOID Get();
    ULONG GetSize() const;
    void Release();
    DWORD GetAccessTime() const;
    const char *GetFileName();

    void SetPlayTime(DWORD dwPlayTime);
    DWORD GetPlayTime() const;

  protected:
    bool ReadFromDisk();
    void Destroy();

    char m_filename[128]{};
    int m_iRefCount = 0;
    DWORD m_dwAccessTime;
    DWORD m_dwPlayTime = 0;
    ULONG m_size = 0;
    LPVOID m_data;
    long m_flag = 0;
    bool m_assigned = false;

  private:
    static U32 AILCALLBACK open_callback(char const *filename, U32 *file_handle);
    static void AILCALLBACK close_callback(U32 file_handle);
    static S32 AILCALLBACK seek_callback(U32 file_handle, S32 offset, U32 type);
    static U32 AILCALLBACK read_callback(U32 file_handle, void *buffer, U32 bytes);

    static bool isSlotIndex(DWORD dwIndex);
    static int GetEmptySlotIndex();

    static bool ms_isSoundFile[SOUND_FILE_MAX_NUM];
    static std::stringstream ms_SoundFile[SOUND_FILE_MAX_NUM];
};

#endif