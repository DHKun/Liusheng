#pragma once

#include <cstddef>
#include <cstdint>
#include <memory>

#include <QtCore/QByteArray>
#include <QtCore/QVector>
#include <QtGui/QGuiApplication>
#include <QtWidgets/QApplication>

#include "cxx-qt-lib/qcoreapplication.h"
#include "rust/cxx.h"

namespace liusheng {

inline std::unique_ptr<QGuiApplication>
newQApplication(rust::Slice<const std::uint8_t> encodedArgs)
{
  QVector<QByteArray> args;
  const auto* data = reinterpret_cast<const char*>(encodedArgs.data());
  std::size_t start = 0;
  for (std::size_t index = 0; index < encodedArgs.size(); ++index) {
    if (encodedArgs[index] != 0) {
      continue;
    }
    args.append(QByteArray(data + start,
                           static_cast<qsizetype>(index - start)));
    start = index + 1;
  }
  if (start < encodedArgs.size()) {
    args.append(QByteArray(data + start,
                           static_cast<qsizetype>(encodedArgs.size() - start)));
  }
  if (args.isEmpty()) {
    args.append(QByteArrayLiteral("liusheng"));
  }

  auto* argsData = new rust::cxxqtlib1::ApplicationArgsData(args);
  auto application =
    std::make_unique<QApplication>(argsData->size(), argsData->data());
  Q_ASSERT(application != nullptr);
  argsData->setParent(application.get());
  return application;
}

} // namespace liusheng
