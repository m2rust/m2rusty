#pragma once

#include <string>
#include <set>
#include <map>

#include "FileProvider.h"

class Folder : public FileProvider
{
protected:
	std::string folderPath;
	std::set<std::string> fileList = {};
	std::map<std::string, std::string> realPathMap = {};

public:
	Folder(const std::string& folderPath);
	std::vector<std::string> listFiles() override;
	bool fileExists(const std::string& fileName) override;
	bool getFile(const std::string& fileName, std::shared_ptr<std::vector<char>>& fileData) override;

private:
	void ListFiles(const std::string& directory, const std::string& relativePath);
};
