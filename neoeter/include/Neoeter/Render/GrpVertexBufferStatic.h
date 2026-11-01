#pragma once

#include "GrpVertexBuffer.h"

class CStaticVertexBuffer : public CGraphicVertexBuffer
{
  public:
    CStaticVertexBuffer();
    ~CStaticVertexBuffer() override;

    bool Create(int vtxCount, DWORD fvf, bool isManaged = true);
};
