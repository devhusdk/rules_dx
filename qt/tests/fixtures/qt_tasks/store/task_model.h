#pragma once

#include <QAbstractListModel>
#include <QHash>
#include <QString>
#include <QVector>
#include <QtQmlIntegration/qqmlintegration.h>

struct Task {
  QString title;
  QString asset;
  bool done = false;
};

class TaskModel : public QAbstractListModel {
  Q_OBJECT
  QML_ELEMENT
  Q_PROPERTY(int count READ count NOTIFY countChanged)
  Q_PROPERTY(int remaining READ remaining NOTIFY remainingChanged)
  Q_PROPERTY(QString path READ path WRITE setPath NOTIFY pathChanged)

public:
  enum Role { TitleRole = Qt::UserRole + 1, AssetRole, DoneRole };
  Q_ENUM(Role)

  using QAbstractListModel::QAbstractListModel;

  int rowCount(const QModelIndex &parent = QModelIndex()) const override;
  QVariant data(const QModelIndex &index, int role) const override;
  QHash<int, QByteArray> roleNames() const override;

  int count() const;
  int remaining() const;
  QString path() const;
  void setPath(const QString &path);

  Q_INVOKABLE void add(const QString &title, const QString &asset);
  Q_INVOKABLE bool toggle(int row);
  Q_INVOKABLE bool remove(int row);
  Q_INVOKABLE QString title(int row) const;
  Q_INVOKABLE QString asset(int row) const;
  Q_INVOKABLE bool isDone(int row) const;
  Q_INVOKABLE bool load();
  Q_INVOKABLE bool save();

signals:
  void countChanged();
  void remainingChanged();
  void pathChanged();
  void failed(const QString &reason);

private:
  bool valid(int row) const;
  void reset(const QVector<Task> &tasks);

  QVector<Task> tasks_;
  QString path_;
};
