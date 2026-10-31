#ifdef NDEBUG
#undef NDEBUG
#endif

#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/IO/Folder.h>
#include <Neoeter/IO/Inline.h>
#include <Neoeter/IO/ZIP.h>

#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <string>
#include <zip.h>

#define TEST_ASSERT(cond)                                                      \
  do {                                                                         \
    if (!(cond)) {                                                             \
      fprintf(stderr, "[FAIL] %s:%d: Assertion '%s' failed\n", __FILE__,       \
              __LINE__, #cond);                                                \
      std::abort();                                                            \
    }                                                                          \
  } while (0)

namespace fs = std::filesystem;

static void CreateSampleZip(const std::string &zipPath) {
  int err = 0;
  zip_t *za = zip_open(zipPath.c_str(), ZIP_CREATE | ZIP_TRUNCATE, &err);
  TEST_ASSERT(za != nullptr);

  const char *content1 = "ZIP_ASSET_DATA_12345";
  zip_source_t *s1 = zip_source_buffer(za, content1, strlen(content1), 0);
  TEST_ASSERT(s1 != nullptr);
  zip_int64_t idx1 = zip_file_add(za, "ymir work/ui/window.dds", s1,
                                  ZIP_FL_OVERWRITE | ZIP_FL_ENC_UTF_8);
  TEST_ASSERT(idx1 >= 0);

  const char *content2 = "ZIP_LOCALE_TEXT_HELLO";
  zip_source_t *s2 = zip_source_buffer(za, content2, strlen(content2), 0);
  TEST_ASSERT(s2 != nullptr);
  zip_int64_t idx2 = zip_file_add(za, "locale/en/locale_game.txt", s2,
                                  ZIP_FL_OVERWRITE | ZIP_FL_ENC_UTF_8);
  TEST_ASSERT(idx2 >= 0);

  int closeRes = zip_close(za);
  TEST_ASSERT(closeRes == 0);
}

int main() {
  printf("[test_eterpack] Running EterPack VFS unit tests...\n");

  CEterPackManager packMgr;

  // 1. Test ConvertFileName & inlineConvertPackFilename
  {
    std::string converted =
        packMgr.ConvertFileName("D:\\Ymir Work\\UI\\Game.DDS");
    TEST_ASSERT(converted == "d:/ymir work/ui/game.dds");

    char buf[] = "D:\\Ymir Work\\PC\\Warrior\\Action.GR2";
    inlineConvertPackFilename(buf);
    TEST_ASSERT(std::string(buf) == "d:/ymir work/pc/warrior/action.gr2");
    printf("  [PASS] Filename normalization & case folding\n");
  }

  // Prepare temporary test sandbox in /tmp
  fs::path tempRoot = fs::temp_directory_path() / "eterpack_test_sandbox";
  fs::remove_all(tempRoot);
  fs::create_directories(tempRoot / "pack_folder" / "ymir work" / "item" /
                         "weapon");
  fs::create_directories(tempRoot / "direct_files");

  // Create sample files inside FOLDER provider (including mixed case on disk)
  const std::string swordContent = "GR2_SWORD_MESH_PAYLOAD";
  {
    std::ofstream f(tempRoot / "pack_folder" / "ymir work" / "item" / "weapon" /
                        "Sword_001.gr2",
                    std::ios::binary);
    f << swordContent;
  }

  // Create a direct disk file
  const std::string directContent = "DIRECT_CONFIG_FILE_CONTENT";
  fs::path directFilePath = tempRoot / "direct_files" / "AtlasInfo.txt";
  {
    std::ofstream f(directFilePath, std::ios::binary);
    f << directContent;
  }

  // 2. Test FOLDER Pack Provider
  {
    std::string folderPackPath = (tempRoot / "pack_folder").string();
    TEST_ASSERT(packMgr.PackExists(folderPackPath, "FOLDER"));
    TEST_ASSERT(packMgr.RegisterPack(folderPackPath, "FOLDER"));

    // Lookup with Windows backslashes and mixed case
    const char *virtualSwordPath = "D:\\Ymir Work\\Item\\Weapon\\SWORD_001.GR2";
    TEST_ASSERT(packMgr.isExistInPack(virtualSwordPath));
    TEST_ASSERT(packMgr.isExist(virtualSwordPath));

    CEterPackManager::TPackDataPtr dataPtr;
    TEST_ASSERT(packMgr.Get(virtualSwordPath, dataPtr));
    TEST_ASSERT(dataPtr != nullptr);
    std::string loaded(dataPtr->begin(), dataPtr->end());
    TEST_ASSERT(loaded == swordContent);

    std::stringstream ss;
    TEST_ASSERT(packMgr.Get(virtualSwordPath, ss));
    TEST_ASSERT(ss.str() == swordContent);
    printf("  [PASS] FOLDER provider registration & case-insensitive virtual "
           "lookup\n");
  }

  // 3. Test ZIP Pack Provider
  {
    std::string zipBasePath = (tempRoot / "sample_pack").string();
    CreateSampleZip(zipBasePath + ".zip");

    TEST_ASSERT(packMgr.PackExists(zipBasePath, "ZIP"));
    TEST_ASSERT(packMgr.RegisterPack(zipBasePath, "ZIP"));

    // Verify "ymir work/" -> "d:/ymir work/" virtual mapping inside ZIP
    const char *virtualUiPath = "d:\\YMIR WORK\\ui\\Window.dds";
    TEST_ASSERT(packMgr.isExistInPack(virtualUiPath));

    CEterPackManager::TPackDataPtr zipData;
    TEST_ASSERT(packMgr.Get(virtualUiPath, zipData));
    TEST_ASSERT(zipData != nullptr);
    std::string uiLoaded(zipData->begin(), zipData->end());
    TEST_ASSERT(uiLoaded == "ZIP_ASSET_DATA_12345");

    // Verify standard relative path inside ZIP
    CEterPackManager::TPackDataPtr localeData;
    TEST_ASSERT(packMgr.Get("LOCALE\\EN\\Locale_Game.txt", localeData));
    TEST_ASSERT(localeData != nullptr);
    std::string localeLoaded(localeData->begin(), localeData->end());
    TEST_ASSERT(localeLoaded == "ZIP_LOCALE_TEXT_HELLO");
    printf("  [PASS] ZIP provider archive reading & virtual path resolution\n");
  }

  // 4. Test Direct File Access & Search Modes
  {
    packMgr.SetSearchMode(false); // SEARCH_FILE_FIRST
    TEST_ASSERT(packMgr.GetSearchMode() == CEterPackManager::SEARCH_FILE_FIRST);

    std::string directPathStr = directFilePath.string();
    TEST_ASSERT(packMgr.isExist(directPathStr.c_str()));

    CEterPackManager::TPackDataPtr directData;
    TEST_ASSERT(packMgr.Get(directPathStr, directData));
    TEST_ASSERT(directData != nullptr);
    std::string loaded(directData->begin(), directData->end());
    TEST_ASSERT(loaded == directContent);

    packMgr.SetSearchMode(true); // SEARCH_PACK_FIRST
    TEST_ASSERT(packMgr.GetSearchMode() == CEterPackManager::SEARCH_PACK_FIRST);
    TEST_ASSERT(!packMgr.isExist("d:/ymir work/non_existent_file.abc"));
    printf("  [PASS] Direct file lookup & search mode switching\n");
  }

  // Clean up temporary directory
  fs::remove_all(tempRoot);

  printf("[test_eterpack] All tests passed successfully!\n");
  return 0;
}
