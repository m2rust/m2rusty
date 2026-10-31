#include <Neoeter/IO/Folder.h>
#include <Neoeter/IO/EterPackManager.h>

#include <iostream>
#include <fstream>
#include <iterator>
#include <stdexcept>
#include <filesystem>

namespace fs = std::filesystem;

void Folder::ListFiles(const std::string& directory, const std::string& relativePath)
{
    std::error_code ec;
    if (!fs::exists(directory, ec) || !fs::is_directory(directory, ec))
    {
        throw std::runtime_error("Failed to open directory: " + directory);
    }

    for (const auto& entry : fs::directory_iterator(directory, ec))
    {
        if (ec)
            break;

        const std::string fileOrDirName = entry.path().filename().string();
        if (fileOrDirName == "." || fileOrDirName == "..")
            continue;

        std::string fullPath = entry.path().string();
        std::string currentRelativePath = relativePath + fileOrDirName;

        if (entry.is_directory(ec))
        {
            ListFiles(fullPath, currentRelativePath + "/");
        }
        else
        {
            std::string normalizedPath = CEterPackManager::Instance().ConvertFileName(currentRelativePath);

            if (normalizedPath.rfind("ymir work/", 0) == 0)
            {
                normalizedPath = "d:/" + normalizedPath;
            }

            this->fileList.insert(normalizedPath);
            this->realPathMap[normalizedPath] = fullPath;
        }
    }
}

Folder::Folder(const std::string& folderPath)
{
    this->folderPath = folderPath;
    ListFiles(folderPath, "");
}

std::vector<std::string> Folder::listFiles()
{
    std::vector<std::string> result;
    std::copy(fileList.begin(), fileList.end(), std::back_inserter(result));
    return result;
}

bool Folder::fileExists(const std::string& fileName)
{
    std::string normalized = CEterPackManager::Instance().ConvertFileName(fileName);
    return fileList.find(normalized) != fileList.end();
}

bool Folder::getFile(const std::string& fileName, std::shared_ptr<std::vector<char>>& fileData)
{
    std::string normalized = CEterPackManager::Instance().ConvertFileName(fileName);

    std::string diskPath;
    auto mapIt = realPathMap.find(normalized);
    if (mapIt != realPathMap.end())
    {
        diskPath = mapIt->second;
    }
    else
    {
        std::string realFileName = normalized;
        const std::string ymirPrefix = "d:/";
        if (realFileName.rfind(ymirPrefix, 0) == 0)
            realFileName = realFileName.substr(ymirPrefix.length());

        diskPath = this->folderPath + "/" + realFileName;
    }

    // Try to open the file
    std::ifstream file(diskPath, std::ios::binary);
    if (!file.is_open())
        return false;

    // Read the file's contents
    fileData = std::make_shared<std::vector<char>>(
        std::istreambuf_iterator<char>(file),
        std::istreambuf_iterator<char>()
    );
    return true;
}
