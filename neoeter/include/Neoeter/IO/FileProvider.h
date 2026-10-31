#pragma once

#include <vector>
#include <string>
#include <sstream>
#include <memory>

class FileProvider
{
public:
	virtual ~FileProvider() = default;
	virtual std::vector<std::string> listFiles() = 0;
	virtual bool fileExists(const std::string& fileName) = 0;
	virtual bool getFile(const std::string& fileName, std::shared_ptr<std::vector<char>>& fileData) = 0;
};
