#include "examples/qt-tasks/catalog/asset_catalog.h"

#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

AssetCatalog::AssetCatalog(QObject *parent) : QObject(parent) {
  QFile file(QStringLiteral(":/catalog/assets.json"));
  if (!file.open(QIODevice::ReadOnly)) {
    return;
  }
  const QJsonDocument document = QJsonDocument::fromJson(file.readAll());
  for (const QJsonValue &value : document.array()) {
    const QJsonObject object = value.toObject();
    assets_.insert(object.value(QStringLiteral("id")).toString(),
                   Asset{object.value(QStringLiteral("kind")).toString(),
                         object.value(QStringLiteral("name")).toString()});
  }
  loaded_ = !assets_.isEmpty();
}

QStringList AssetCatalog::ids() const {
  QStringList ids = assets_.keys();
  ids.sort();
  return ids;
}

bool AssetCatalog::loaded() const { return loaded_; }

bool AssetCatalog::contains(const QString &id) const { return assets_.contains(id); }

QString AssetCatalog::kind(const QString &id) const { return assets_.value(id).kind; }

QString AssetCatalog::name(const QString &id) const { return assets_.value(id).name; }
