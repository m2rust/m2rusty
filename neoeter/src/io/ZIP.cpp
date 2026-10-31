#include <Neoeter/IO/ZIP.h>
#include <Neoeter/IO/EterPackManager.h>

#include <iostream>
#include <fstream>
#include <stdexcept>

ZIP::ZIP(const std::string& archivePath)
{
    int err = 0;

    if ((zipFile = zip_open(archivePath.c_str(), 0, &err)) == nullptr) {
        zip_error_t error;

        zip_error_init_with_code(&error, err);
        std::string errorMessage = zip_error_strerror(&error);
        zip_error_fini(&error);

        throw std::runtime_error("Cannot open ZIP archive '" + archivePath + "': " + errorMessage);
    }

    // Read the list of files in the archive
    zip_int64_t numEntries = zip_get_num_entries(zipFile, 0);
    for (zip_int64_t index = 0; index < numEntries; index++)
    {
        zip_stat_t fileData;
        if (zip_stat_index(zipFile, static_cast<zip_uint64_t>(index), 0, &fileData) != 0)
            continue;

        if (!fileData.name)
            continue;

        std::string fileName(fileData.name);

        // Skip directory entries
        if (fileName.empty() || fileName.back() == '/' || fileName.back() == '\\') {
            continue;
        }

        fileName = CEterPackManager::Instance().ConvertFileName(fileName);

        if (fileName.rfind("ymir work/", 0) == 0) {
            fileName = "d:/" + fileName;
        }

        fileList.insert({ fileName, fileData.index });
    }
}

ZIP::~ZIP()
{
    if (zipFile)
    {
        zip_close(zipFile);
        zipFile = nullptr;
    }
}

std::vector<std::string> ZIP::listFiles()
{
    std::vector<std::string> result;
    for (auto const& file : fileList)
        result.push_back(file.first);
    return result;
}

bool ZIP::fileExists(const std::string& fileName)
{
    std::string normalized = CEterPackManager::Instance().ConvertFileName(fileName);
    auto it = fileList.find(normalized);
    if (it == fileList.end())
        return false;

    return true;
}

bool ZIP::getFile(const std::string& fileName, std::shared_ptr<std::vector<char>>& fileData)
{
    std::string normalized = CEterPackManager::Instance().ConvertFileName(fileName);
    auto it = fileList.find(normalized);
    if (it == fileList.end())
        return false;

    auto file = zip_fopen_index(zipFile, it->second, 0);
    if (file == nullptr)
        return false;

    zip_stat_t fileInfo;
    if (zip_stat_index(zipFile, it->second, 0, &fileInfo) != 0)
    {
        zip_fclose(file);
        return false;
    }

    // Initialize the buffer
    fileData = std::make_shared<std::vector<char>>(fileInfo.size);

    // Attempt to read the data into the buffer
    zip_int64_t retval = 0;
    if (fileInfo.size > 0)
    {
        retval = zip_fread(file, fileData->data(), fileData->size());
    }

    // Close the file
    zip_fclose(file);

    if (retval == -1)
        return false;

    return true;
}
