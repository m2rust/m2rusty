#include <Neoeter/Image/StdAfx.h>

#include <cassert>
#include <cstdio>
#include <cstring>
#include <print>

#if __has_include(<Neoeter/Platform/Base/MappedFile.h>)
#include <Neoeter/Platform/Base/MappedFile.h>
#endif
#include <Neoeter/Image/TGAImage.h>

CTGAImage::CTGAImage() : m_dwFlag(0), m_pdwEndPtr(nullptr)
{
    memset(&m_Header, 0, sizeof(m_Header));
}

CTGAImage::~CTGAImage() = default;

CTGAImage::CTGAImage(CImage &image) : m_dwFlag(0), m_pdwEndPtr(nullptr)
{
    int const w = image.GetWidth();
    int const h = image.GetHeight();

    Create(w, h);

    DWORD *pdwDest = GetBasePointer();
    memcpy(pdwDest, image.GetBasePointer(), w * h * sizeof(DWORD));
    FlipTopToBottom();
}

void CTGAImage::Create(int width, int height)
{
    memset(&m_Header, 0, sizeof(m_Header));

    m_Header.imgType = 2;
    m_Header.width = static_cast<short>(width);
    m_Header.height = static_cast<short>(height);
    m_Header.colorBits = 32;
    m_Header.desc = 0x08; // alpha channel 있음

    CImage::Create(width, height);
    m_pdwEndPtr = GetBasePointer() + (width * height);
}

bool CTGAImage::LoadFromMemory(int iSize, const BYTE *c_pbMem)
{
    if (iSize < 18 || !c_pbMem)
        return false;

    memcpy(&m_Header, c_pbMem, 18);
    c_pbMem += 18;
    iSize -= 18;

    CImage::Create(m_Header.width, m_Header.height);
    m_pdwEndPtr = GetBasePointer() + (m_Header.width * m_Header.height);

    UINT const hxw = m_Header.width * m_Header.height;
    BYTE r = 0;
    BYTE g = 0;
    BYTE b = 0;
    BYTE a = 0;
    DWORD i = 0;

    DWORD *pdwDest = GetBasePointer();

    switch (m_Header.imgType)
    {
    case 3: // 알파만 있는 것 (1bytes per pixel, 거의 안쓰임)
    {
        for (i = 0; i < hxw; ++i)
        {
            a = static_cast<char>(*(c_pbMem++));
            pdwDest[i] = (a << 24) | (a << 16) | (a << 8) | a;
        }
    }
    break;

    case 2: // 압축 안된 TGA
    {
        if (m_Header.colorBits == 16) // 16bit
        {
            for (i = 0; i < hxw; ++i)
            {
                WORD w = 0;

                memcpy(&w, c_pbMem, sizeof(WORD));
                c_pbMem += sizeof(WORD);
                iSize -= sizeof(WORD);

                b = static_cast<BYTE>(w & 0x1F);
                g = static_cast<BYTE>((w >> 5) & 0x1F);
                r = static_cast<BYTE>((w >> 10) & 0x1F);

                b <<= 3;
                g <<= 3;
                r <<= 3;
                a = 0xff;

                pdwDest[i] = (a << 24) | (r << 16) | (g << 8) | b;
            }
        }
        else if (m_Header.colorBits == 24) // 24bit
        {
            for (i = 0; i < hxw; ++i)
            {
                r = *(c_pbMem++);
                --iSize;
                g = *(c_pbMem++);
                --iSize;
                b = *(c_pbMem++);
                --iSize;
                a = 0xff;

                pdwDest[i] = (a << 24) | (r << 16) | (g << 8) | b;
            }
        }
        else if (m_Header.colorBits == 32) // 32bit
        {
            int size = GetWidth();
            size *= GetHeight() * 4;

            memcpy(pdwDest, c_pbMem, size);
            c_pbMem += size;
            iSize -= size;
        }
    }
    break;

    case 10: // 압축 된 TGA (RLE)
    {
        BYTE rle = 0;

        if (m_Header.colorBits == 24)
        {
            i = 0;
            while (i < hxw)
            {
                rle = *(c_pbMem++);
                --iSize;

                if (rle < 0x80) // 압축 안된 곳
                {
                    rle++;

                    while (rle)
                    {
                        b = *(c_pbMem++);
                        --iSize;
                        g = *(c_pbMem++);
                        --iSize;
                        r = *(c_pbMem++);
                        --iSize;
                        a = 0xff;
                        pdwDest[i++] = (a << 24) | (r << 16) | (g << 8) | b;

                        if (i > hxw)
                        {
                            assert(!"RLE overflow");
                            std::print("RLE overflow");
                            return false;
                        }
                        --rle;
                    }
                }
                else
                {
                    // 압축 된 곳
                    rle -= 127;

                    b = *(c_pbMem++);
                    --iSize;
                    g = *(c_pbMem++);
                    --iSize;
                    r = *(c_pbMem++);
                    --iSize;
                    a = 0xff;

                    while (rle)
                    {
                        pdwDest[i++] = (a << 24) | (r << 16) | (g << 8) | b;

                        if (i > hxw)
                        {
                            assert(!"RLE overflow");
                            std::print("RLE overflow");
                            return false;
                        }
                        --rle;
                    }
                }
            }
        }
        else if (m_Header.colorBits == 32)
        {
            i = 0;
            while (i < hxw)
            {
                rle = *(c_pbMem++);
                --iSize;

                if (rle < 0x80)
                {
                    rle++;

                    while (rle)
                    {
                        b = *(c_pbMem++);
                        --iSize;
                        g = *(c_pbMem++);
                        --iSize;
                        r = *(c_pbMem++);
                        --iSize;
                        a = *(c_pbMem++);
                        --iSize;
                        pdwDest[i++] = (a << 24) | (r << 16) | (g << 8) | b;

                        if (i > hxw)
                        {
                            assert(!"RLE overflow");
                            std::print("RLE overflow");
                            return false;
                        }
                        --rle;
                    }
                }
                else
                {
                    rle -= 127;

                    b = *(c_pbMem++);
                    --iSize;
                    g = *(c_pbMem++);
                    --iSize;
                    r = *(c_pbMem++);
                    --iSize;
                    a = *(c_pbMem++);
                    --iSize;

                    while (rle)
                    {
                        pdwDest[i++] = (a << 24) | (r << 16) | (g << 8) | b;

                        if (i > hxw)
                        {
                            assert(!"RLE overflow");
                            std::print("RLE overflow");
                            return false;
                        }

                        --rle;
                    }
                }
            }
        }
    }
    break;
    }

    if (!(m_Header.desc & 0x20))
    {
        FlipTopToBottom();
    }

    return true;
}

int CTGAImage::GetRLEPixelCount(const DWORD *data)
{
    int r = 1;
    DWORD pixel = 0;

    if (data >= m_pdwEndPtr)
        return 0;

    pixel = *data;

    while ((r < 127) && (data < m_pdwEndPtr - 1))
    {
        if (pixel != *(++data))
            return r;

        r++;
    }

    return r;
}

int CTGAImage::GetRawPixelCount(const DWORD *data)
{
    int i = 0;

    if (data >= m_pdwEndPtr)
        return 0;

    while ((data < m_pdwEndPtr) && (i < 127))
    {
        int const rle = GetRLEPixelCount(data);

        if (rle >= 4)
            break;

        data++;
        i++;
    }

    return i;
}

void CTGAImage::SetCompressed(bool isCompress)
{
    if (isCompress)
    {
        m_Header.imgType = 10;
    }
    else
    {
        m_Header.imgType = 2;
    }
}

void CTGAImage::SetAlphaChannel(bool isExist)
{
    if (isExist)
    {
        m_Header.desc |= 0x08;
    }
    else
    {
        m_Header.desc &= ~0x08;
    }
}

bool CTGAImage::SaveToDiskFile(const char *c_szFileName)
{
    FILE *fp = fopen(c_szFileName, "wb");

    if (!fp)
        return false;

    fwrite(&m_Header, 18, 1, fp);

    if (m_Header.imgType == 10) // RLE 압축으로 저장
    {
        DWORD *data = GetBasePointer();
        m_pdwEndPtr = data + (GetWidth() * GetHeight());

        while (data < m_pdwEndPtr)
        {
            int const rle = GetRLEPixelCount(data);

            if (rle < 4)
            {
                int raw = GetRawPixelCount(data);

                if (raw == 0)
                    break;

                fputc(raw - 1, fp);

                while (raw)
                {
                    fwrite(data, sizeof(DWORD), 1, fp);
                    data++;
                    raw--;
                }
            }
            else
            {
                fputc((rle - 1) | 0x80, fp);
                fwrite(data, sizeof(DWORD), 1, fp);
                data += rle;
            }
        }
    }
    else
    {
        int size = GetWidth();
        size *= GetHeight() * 4;
        fwrite(GetBasePointer(), size, 1, fp);
    }

    fclose(fp);
    return true;
}

TGA_HEADER &CTGAImage::GetHeader()
{
    return m_Header;
}
