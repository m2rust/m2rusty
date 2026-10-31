#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cassert>
#include <unistd.h>
#include <Neoeter/Platform/Base/FileBase.h>
#include <Neoeter/Platform/Base/MappedFile.h>

int main()
{
    printf("[TEST] Starting FileBase & MappedFile tests...\n");

    const char* testPath = "/tmp/mt2_test_filebase.dat";
    const char* sampleData = "Metin2 Linux Porting - File I/O Verification Data Block";
    const int sampleLen = static_cast<int>(strlen(sampleData));

    // 1. Write file with CFileBase
    {
        CFileBase file;
        BOOL created = file.Create(testPath, CFileBase::FILEMODE_WRITE);
        assert(created == TRUE);

        BOOL written = file.Write(sampleData, sampleLen);
        assert(written == TRUE);
        file.Close();
    }
    printf("  [PASS] CFileBase created and wrote %d bytes.\n", sampleLen);

    // 2. Read back with CFileBase
    {
        CFileBase file;
        BOOL opened = file.Create(testPath, CFileBase::FILEMODE_READ);
        assert(opened == TRUE);
        assert(static_cast<int>(file.Size()) == sampleLen);

        char readBuffer[128] = {0};
        BOOL readOk = file.Read(readBuffer, sampleLen);
        assert(readOk == TRUE);
        assert(memcmp(readBuffer, sampleData, sampleLen) == 0);
        file.Close();
    }
    printf("  [PASS] CFileBase read back and verified identical contents.\n");

    // 3. Map file into memory using CMappedFile
    {
        CMappedFile mapped;
        const void* pMappedData = nullptr;
        BOOL mapCreated = mapped.Create(testPath, &pMappedData, 0, 0);
        assert(mapCreated == TRUE);
        assert(pMappedData != nullptr);
        assert(static_cast<int>(mapped.Size()) == sampleLen);
        assert(memcmp(pMappedData, sampleData, sampleLen) == 0);
        mapped.Destroy();
    }
    printf("  [PASS] CMappedFile mapped file to memory successfully.\n");

    // Cleanup
    unlink(testPath);
    printf("[TEST] FileBase & MappedFile tests PASSED successfully.\n");
    return 0;
}
