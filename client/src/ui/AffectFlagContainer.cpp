#include <Client/UI/StdAfx.h>
#include <Client/UI/AffectFlagContainer.h>

CAffectFlagContainer::CAffectFlagContainer()
{
    Clear();
}

void CAffectFlagContainer::Clear()
{
    memset(m_aElement, 0, sizeof(m_aElement));
}

void CAffectFlagContainer::CopyInstance(const CAffectFlagContainer &c_rkAffectContainer)
{
    memcpy(m_aElement, c_rkAffectContainer.m_aElement, sizeof(m_aElement));
}

void CAffectFlagContainer::CopyData(UINT uPos, UINT uByteSize, const void *c_pvData)
{
    const BYTE *c_pbData = static_cast<const BYTE *>(c_pvData);
    Element bMask = 0x01;

    UINT const uBitEnd = uPos + (uByteSize * 8);
    for (UINT i = uPos; i < uBitEnd; ++i)
    {
        Set(i, (*c_pbData & bMask) != 0);
        bMask <<= 1;

        if (bMask == 0)
        {
            ++c_pbData;
            bMask = 0x01;
        }
    }
}

void CAffectFlagContainer::ConvertToPosition(unsigned *uRetX, unsigned *uRetY) const
{
    auto *pos = reinterpret_cast<const DWORD *>(m_aElement);
    *uRetX = pos[0];
    *uRetY = pos[1];
}
/*
const void * CAffectFlagContainer::GetDataPtr(UINT uPos) const
{
    if (uPos/8>=BYTE_SIZE)
    {
        return NULL;
    }

    return (const void *)&m_aElement[uPos];
}
*/

void CAffectFlagContainer::Set(UINT uPos, bool isSet)
{
    if (uPos / 8 >= BYTE_SIZE)
    {
        TraceError("CAffectFlagContainer::Set(uPos=%d>%d, isSet=%d", uPos, BYTE_SIZE * 8, isSet);
        return;
    }

    BYTE &rElement = m_aElement[uPos / 8];

    BYTE const bMask = static_cast<BYTE>(1 << (uPos & 7));
    if (isSet)
    {
        rElement |= bMask;
    }
    else
    {
        rElement &= ~bMask;
    }
}

bool CAffectFlagContainer::IsSet(UINT uPos) const
{
    if (uPos / 8 >= BYTE_SIZE)
    {
        TraceError("CAffectFlagContainer::IsSet(uPos=%d>%d", uPos, BYTE_SIZE * 8);
        return false;
    }

    const BYTE &c_rElement = m_aElement[uPos / 8];

    BYTE const bMask = static_cast<BYTE>(1 << (uPos & 7));
    return (c_rElement & bMask) != 0;
}
