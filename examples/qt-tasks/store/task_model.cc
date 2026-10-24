#include "examples/qt-tasks/store/task_model.h"

#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSaveFile>

#include <algorithm>

int TaskModel::rowCount(const QModelIndex &parent) const {
  return parent.isValid() ? 0 : static_cast<int>(tasks_.size());
}

QVariant TaskModel::data(const QModelIndex &index, int role) const {
  if (!index.isValid() || !valid(index.row())) {
    return {};
  }
  const Task &task = tasks_.at(index.row());
  switch (role) {
  case Qt::DisplayRole:
  case TitleRole:
    return task.title;
  case AssetRole:
    return task.asset;
  case DoneRole:
    return task.done;
  default:
    return {};
  }
}

QHash<int, QByteArray> TaskModel::roleNames() const {
  return {{TitleRole, "title"}, {AssetRole, "asset"}, {DoneRole, "done"}};
}

int TaskModel::count() const { return static_cast<int>(tasks_.size()); }

int TaskModel::remaining() const {
  return static_cast<int>(
      std::count_if(tasks_.cbegin(), tasks_.cend(),
                    [](const Task &task) { return !task.done; }));
}

QString TaskModel::path() const { return path_; }

void TaskModel::setPath(const QString &path) {
  if (path_ == path) {
    return;
  }
  path_ = path;
  emit pathChanged();
}

void TaskModel::add(const QString &title, const QString &asset) {
  const int row = count();
  beginInsertRows(QModelIndex(), row, row);
  tasks_.append(Task{title, asset, false});
  endInsertRows();
  emit countChanged();
  emit remainingChanged();
}

bool TaskModel::toggle(int row) {
  if (!valid(row)) {
    return false;
  }
  tasks_[row].done = !tasks_[row].done;
  const QModelIndex changed = index(row);
  emit dataChanged(changed, changed, {DoneRole});
  emit remainingChanged();
  return true;
}

bool TaskModel::remove(int row) {
  if (!valid(row)) {
    return false;
  }
  const bool open = !tasks_.at(row).done;
  beginRemoveRows(QModelIndex(), row, row);
  tasks_.removeAt(row);
  endRemoveRows();
  emit countChanged();
  if (open) {
    emit remainingChanged();
  }
  return true;
}

QString TaskModel::title(int row) const {
  return valid(row) ? tasks_.at(row).title : QString();
}

QString TaskModel::asset(int row) const {
  return valid(row) ? tasks_.at(row).asset : QString();
}

bool TaskModel::isDone(int row) const {
  return valid(row) && tasks_.at(row).done;
}

bool TaskModel::load() {
  QFile file(path_);
  if (!file.exists()) {
    reset({});
    return true;
  }
  if (!file.open(QIODevice::ReadOnly)) {
    emit failed(path_ + QStringLiteral(": ") + file.errorString());
    return false;
  }
  QJsonParseError error;
  const QJsonDocument document =
      QJsonDocument::fromJson(file.readAll(), &error);
  if (error.error != QJsonParseError::NoError || !document.isArray()) {
    emit failed(path_ + QStringLiteral(": not a task list"));
    return false;
  }
  QVector<Task> tasks;
  for (const QJsonValue &value : document.array()) {
    const QJsonObject object = value.toObject();
    tasks.append(Task{object.value(QStringLiteral("title")).toString(),
                      object.value(QStringLiteral("asset")).toString(),
                      object.value(QStringLiteral("done")).toBool()});
  }
  reset(tasks);
  return true;
}

bool TaskModel::save() {
  QJsonArray array;
  for (const Task &task : tasks_) {
    QJsonObject object;
    object.insert(QStringLiteral("title"), task.title);
    object.insert(QStringLiteral("asset"), task.asset);
    object.insert(QStringLiteral("done"), task.done);
    array.append(object);
  }
  QSaveFile file(path_);
  if (!file.open(QIODevice::WriteOnly) ||
      file.write(QJsonDocument(array).toJson(QJsonDocument::Indented)) < 0 ||
      !file.commit()) {
    emit failed(path_ + QStringLiteral(": ") + file.errorString());
    return false;
  }
  return true;
}

bool TaskModel::valid(int row) const { return row >= 0 && row < count(); }

void TaskModel::reset(const QVector<Task> &tasks) {
  const int before = count();
  const int open = remaining();
  beginResetModel();
  tasks_ = tasks;
  endResetModel();
  if (before != count()) {
    emit countChanged();
  }
  if (open != remaining()) {
    emit remainingChanged();
  }
}
