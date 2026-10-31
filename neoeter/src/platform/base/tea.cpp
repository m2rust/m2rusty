#include <Neoeter/Platform/Base/StdAfx.h>
#include <Neoeter/Platform/Base/tea.h>
#include <string.h>

#define TEA_ROUND		32
#define DELTA			0x9E3779B9

void tea_code(const uint32_t sz, const uint32_t sy, const uint32_t *key, uint32_t *dest)
{
	uint32_t y = sy, z = sz, sum = 0;
	uint32_t n = TEA_ROUND;
	
	while (n-- > 0)
	{
		y	+= ((z << 4 ^ z >> 5) + z) ^ (sum + key[sum & 3]);
		sum	+= DELTA;
		z	+= ((y << 4 ^ y >> 5) + y) ^ (sum + key[sum >> 11 & 3]);
	}
	
	*(dest++)	= y;
	*dest	= z;
}

void tea_decode(const uint32_t sz, const uint32_t sy, const uint32_t *key, uint32_t *dest)
{
	uint32_t y = sy, z = sz, sum = DELTA * TEA_ROUND;
	uint32_t n = TEA_ROUND;
	
	while (n-- > 0)
	{
		z -= ((y << 4 ^ y >> 5) + y) ^ (sum + key[sum >> 11 & 3]);
		sum -= DELTA;
		y -= ((z << 4 ^ z >> 5) + z) ^ (sum + key[sum & 3]);
	}
	
	*(dest++)	= y;
	*dest	= z;
}

int tea_encrypt(uint32_t *dest, const uint32_t *src, const uint32_t * key, int size)
{
	int resize = (size % 8 != 0) ? size + 8 - (size % 8) : size;

	for (int i = 0; i < (resize >> 3); i++, dest += 2, src += 2)
	{
		if ((i + 1) * 8 <= size)
		{
			tea_code(*(src + 1), *src, key, dest);
		}
		else
		{
			// Zero-pad the tail into a local block: writing the padding back
			// into `src` overflowed the caller's buffer by up to 7 bytes.
			uint32_t block[2] = { 0, 0 };
			const int remaining = size - i * 8;
			memcpy(block, src, remaining);
			tea_code(block[1], block[0], key, dest);
		}
	}

	return resize;
}

int tea_decrypt(uint32_t *dest, const uint32_t *src, const uint32_t * key, int size)
{
	int		i;
	int		resize;
	
	if (size % 8 != 0)
		resize = size + 8 - (size % 8);
	else
		resize = size;
	
	for (i = 0; i < resize >> 3; i++, dest += 2, src += 2)
		tea_decode(*(src + 1), *src, key, dest);
	
	return (resize);
}
