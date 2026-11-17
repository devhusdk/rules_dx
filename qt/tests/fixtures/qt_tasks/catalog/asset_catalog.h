#pragma once

#include <QHash>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QtQmlIntegration/qqmlintegration.h>

class AssetCatalog : public QObject {
  Q_OBJECT
  QML_ELEMENT
  Q_PROPERTY(QStringList ids READ ids CONSTANT)
  Q_PROPERTY(bool loaded READ loaded CONSTANT)

public:
  explicit AssetCatalog(QObject *parent = nullptr);

  QStringList ids() const;
  bool loaded() const;

  Q_INVOKABLE bool contains(const QString &id) const;
  Q_INVOKABLE QString kind(const QString &id) const;
  Q_INVOKABLE QString name(const QString &id) const;

private:
  struct Asset {
    QString kind;
    QString name;
  };

  QHash<QString, Asset> assets_;
  bool loaded_ = false;
};
