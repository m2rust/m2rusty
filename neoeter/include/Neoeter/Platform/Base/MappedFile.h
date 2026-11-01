#ifndef __INC_MAPPEDFILE_H__
#define __INC_MAPPEDFILE_H__

#include "FileBase.h"
#include "lzo.h"

class CMappedFile : public CFileBase
{
  public:
    enum ESeekType
    {
        SEEK_TYPE_BEGIN,
        SEEK_TYPE_CURRENT,
        SEEK_TYPE_END
    };

    CMappedFile();
    ~CMappedFile() override;

    void Link(DWORD dwBufSize, const void *c_pvBufData);

    BOOL Create(const char *filename);
    BOOL Create(const char *filename, const void **dest, int offset, int size);
    LPCVOID Get();
    void Destroy();
    int Seek(DWORD offset, int iSeekType = SEEK_TYPE_BEGIN);
    int Map(const void **dest, int offset = 0, int size = 0);
    DWORD Size() const;
    DWORD GetPosition() const;
    BOOL Read(void *dest, int bytes);
    DWORD GetSeekPosition() const;
    void BindLZObject(CLZObject *pLZObj);
    void BindLZObjectWithBufferedSize(CLZObject *pLZObj);
    BYTE *AppendDataBlock(const void *pBlock, DWORD dwBlockSize);

    BYTE *GetCurrentSeekPoint();

  private:
    void Unmap(LPCVOID data);

    BYTE *m_pbBufLinkData;
    DWORD m_dwBufLinkSize = 0;

    BYTE *m_pbAppendResultDataBlock;
    DWORD m_dwAppendResultDataSize = 0;

    DWORD m_seekPosition = 0;
    [[maybe_unused]] HANDLE m_hFM;
    DWORD m_dataOffset = 0;
    DWORD m_mapSize = 0;
    LPVOID m_lpMapData;
    LPVOID m_lpData;

    CLZObject *m_pLZObj;
};

#endif
