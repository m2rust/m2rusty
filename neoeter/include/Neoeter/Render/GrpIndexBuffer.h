#pragma once

#include "GrpBase.h"

class CGraphicIndexBuffer : public CGraphicBase
{
  public:
    CGraphicIndexBuffer();
    ~CGraphicIndexBuffer() override;

    void Destroy();
    bool Create(int idxCount, D3DFORMAT d3dFmt);
    bool Create(int faceCount, TFace *faces);

    bool CreateDeviceObjects();
    void DestroyDeviceObjects();

    bool Copy(int bufSize, const void *srcIndices);

    bool Lock(void **pretIndices) const;
    void Unlock() const;

    bool Lock(void **pretIndices);
    void Unlock();

    void SetIndices(int startIndex = 0) const;

    LPDIRECT3DINDEXBUFFER9 GetD3DIndexBuffer() const;

    int GetIndexCount() const
    {
        return m_iidxCount;
    }

  protected:
    void Initialize();

    LPDIRECT3DINDEXBUFFER9 m_lpd3dIdxBuf = nullptr;
    DWORD m_dwBufferSize = 0U;
    D3DFORMAT m_d3dFmt;
    int m_iidxCount = 0;
};
