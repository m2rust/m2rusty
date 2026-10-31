#include <support/TestAssert.h>
#include <support/PackFixtures.h>

#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/IO/Folder.h>
#include <Neoeter/IO/Inline.h>
#include <Neoeter/IO/ZIP.h>

#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

namespace fs = std::filesystem;

namespace
{

fs::path makeSandbox()
{
    fs::path root = fs::temp_directory_path() / "openmt2_pack_formats";
    fs::remove_all(root);
    fs::create_directories(root);
    return root;
}

void test_zip_round_trip()
{
    TEST_SECTION("ZIP provider round-trip");

    CEterPackManager manager; // ZIP reaches the singleton for path normalization

    const fs::path root = makeSandbox();
    const fs::path archive = root / "sample.zip";

    const std::vector<char> binary = DeterministicBytes(64 * 1024, 0x12345678u);
    std::vector<char> withNul = {'a', '\0', 'b', '\0', '\0', 'c'};

    std::vector<ZipFixtureEntry> entries = {
        {"ymir work/ui/Window.DDS", {'D', 'D', 'S', ' ', 1, 2, 3, 4}},
        {"locale/en/locale_game.txt", {'h', 'e', 'l', 'l', 'o'}},
        {"data/empty.bin", {}},
        {"data/binary.bin", binary},
        {"data/nul.bin", withNul},
        {"data/directory/", {}},
    };
    TEST_ASSERT(WriteZipFixture(archive.string(), entries));

    ZIP zip(archive.string());
    TEST_ASSERT(zip.listFiles().size() == 5); // directory entry skipped

    TEST_ASSERT(zip.fileExists("d:\\YMIR WORK\\ui\\window.dds"));
    TEST_ASSERT(zip.fileExists("LOCALE/en/Locale_Game.txt"));
    TEST_ASSERT(!zip.fileExists("data/missing.bin"));

    std::shared_ptr<std::vector<char>> data;
    TEST_ASSERT(zip.getFile("DATA/Binary.bin", data));
    TEST_ASSERT(data != nullptr);
    TEST_ASSERT(data->size() == binary.size());
    TEST_ASSERT(std::memcmp(data->data(), binary.data(), binary.size()) == 0);

    data.reset();
    TEST_ASSERT(zip.getFile("data/empty.bin", data));
    TEST_ASSERT(data != nullptr && data->empty());

    data.reset();
    TEST_ASSERT(zip.getFile("data/nul.bin", data));
    TEST_ASSERT(data != nullptr && *data == withNul);

    data.reset();
    TEST_ASSERT(!zip.getFile("data/missing.bin", data));

    fs::remove_all(root);
    TEST_PASS("entries, empty files, embedded NULs and case folding");
}

void test_manager_overlay_and_search_mode()
{
    TEST_SECTION("pack manager overlay");

    const fs::path root = makeSandbox();
    fs::create_directories(root / "folder" / "ymir work" / "ui");
    {
        std::ofstream f(root / "folder" / "ymir work" / "ui" / "Window.dds", std::ios::binary);
        f << "FOLDER";
    }

    const fs::path archive = root / "overlay.zip";
    TEST_ASSERT(WriteZipFixture(archive.string(),
                                {{"ymir work/ui/Window.dds", {'Z', 'I', 'P'}},
                                 {"ymir work/ui/OnlyZip.dds", {'Z'}}}));

    CEterPackManager manager;
    TEST_ASSERT(manager.RegisterPack((root / "folder").string(), "FOLDER"));
    TEST_ASSERT(manager.RegisterPack((root / "overlay").string(), "ZIP"));

    // Direct-file lookup uses the process cwd, so create the virtual path there
    // to give SEARCH_FILE_FIRST something that overlaps the pack entry.
    const fs::path previousCwd = fs::current_path();
    fs::current_path(root);
    fs::create_directories("d:/ymir work/ui");
    {
        std::ofstream f("d:/ymir work/ui/Window.dds", std::ios::binary);
        f << "DISK";
    }

    // SEARCH_FILE_FIRST: the direct file wins for the overlapping name.
    manager.SetSearchMode(false);
    CEterPackManager::TPackDataPtr data;
    TEST_ASSERT(manager.Get("d:/ymir work/ui/Window.dds", data));
    TEST_ASSERT(std::string(data->begin(), data->end()) == "DISK");

    manager.SetSearchMode(true);
    data.reset();
    TEST_ASSERT(manager.Get("d:/ymir work/ui/Window.dds", data));
    TEST_ASSERT(std::string(data->begin(), data->end()) == "ZIP");

    // Everything registered through the manager is still case-insensitive.
    data.reset();
    TEST_ASSERT(manager.Get("D:\\YMIR WORK\\UI\\ONLYZIP.DDS", data));
    TEST_ASSERT(std::string(data->begin(), data->end()) == "Z");
    TEST_ASSERT(manager.isExistInPack("d:/YMIR WORK/ui/onlyzip.dds"));
    TEST_ASSERT(!manager.isExist("d:/ymir work/ui/missing.dds"));

    // Unknown containers are reported as a failed registration, not a crash.
    TEST_ASSERT(!manager.RegisterPack((root / "unknown").string(), "TAR"));

    fs::current_path(previousCwd);
    fs::remove_all(root);
    TEST_PASS("file-first/pack-first precedence and provider overlay");
}

void test_malformed_archives()
{
    TEST_SECTION("malformed ZIP");

    CEterPackManager manager; // used by ZIP once an archive opens
    const fs::path root = makeSandbox();

    {
        const fs::path garbage = root / "garbage.zip";
        std::ofstream f(garbage, std::ios::binary);
        const char bytes[] = "this is definitely not a zip archive";
        f.write(bytes, sizeof(bytes) - 1);
        f.close();

        bool threw = false;
        try
        {
            ZIP zip(garbage.string());
            (void)zip.listFiles();
        }
        catch (const std::runtime_error &)
        {
            threw = true;
        }
        TEST_ASSERT(threw);

        TEST_ASSERT(!manager.RegisterPack((root / "garbage").string(), "ZIP"));
        TEST_ASSERT(manager.GetFileMap().empty());
    }

    {
        const fs::path empty = root / "empty.zip";
        std::ofstream(empty, std::ios::binary).close();

        bool threw = false;
        try
        {
            ZIP zip(empty.string());
            (void)zip.listFiles();
        }
        catch (const std::runtime_error &)
        {
            threw = true;
        }
        TEST_ASSERT(threw);
    }

    {
        // A valid archive truncated in the middle of its central directory.
        const fs::path source = root / "source.zip";
        TEST_ASSERT(WriteZipFixture(source.string(),
                                    {{"a.txt", {'a'}}, {"b.txt", {'b'}}}));
        std::ifstream in(source, std::ios::binary);
        std::vector<char> bytes((std::istreambuf_iterator<char>(in)),
                                std::istreambuf_iterator<char>());
        in.close();
        TEST_ASSERT(bytes.size() > 20);

        const fs::path truncated = root / "truncated.zip";
        std::ofstream out(truncated, std::ios::binary);
        out.write(bytes.data(), static_cast<std::streamsize>(bytes.size() / 2));
        out.close();

        bool threw = false;
        try
        {
            ZIP zip(truncated.string());
            (void)zip.listFiles();
        }
        catch (const std::runtime_error &)
        {
            threw = true;
        }
        TEST_ASSERT(threw);
    }

    fs::remove_all(root);
    TEST_PASS("garbage, empty and truncated archives are rejected cleanly");
}

void test_filename_normalization()
{
    TEST_SECTION("pack filename normalization");

    CEterPackManager manager;
    TEST_ASSERT(manager.ConvertFileName("D:\\Ymir Work\\UI\\Game.DDS") ==
                "d:/ymir work/ui/game.dds");
    TEST_ASSERT(manager.ConvertFileName("") == "");
    TEST_ASSERT(manager.ConvertFileName("MIXED/Path\\Name.TXT") == "mixed/path/name.txt");

    // Non-ASCII bytes are left byte-for-byte; only ASCII case is folded.
    const std::string korean = "\xBF\xEB\xB8\xB6";
    TEST_ASSERT(manager.ConvertFileName(korean) == korean);

    char buffer[] = "D:\\Ymir Work\\PC\\Warrior\\Action.GR2";
    inlineConvertPackFilename(buffer);
    TEST_ASSERT(std::string(buffer) == "d:/ymir work/pc/warrior/action.gr2");

    TEST_PASS("separators, ASCII case and non-ASCII bytes");
}

} // namespace

int main()
{
    test_zip_round_trip();
    test_manager_overlay_and_search_mode();
    test_malformed_archives();
    test_filename_normalization();

    std::printf("[test_pack_formats] all checks passed\n");
    return 0;
}
