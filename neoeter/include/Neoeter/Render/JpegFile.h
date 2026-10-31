#ifndef _JPEGFILE_H_
#define _JPEGFILE_H_

// Screenshot writer. The input buffer is BGR (D3D back-buffer order); the
// implementation converts to RGB and delegates to stb_image_write (T2.2).
int jpeg_save(unsigned char* data, int width, int height, int quality, const char* filename);

#endif
