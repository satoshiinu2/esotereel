#include "camera.h"
#include <QMatrix4x4>
#include <QVector3D>

void applyDirectionalRotation(QMatrix4x4 &mat, Direction direction);

QMatrix4x4 Camera::getViewMat() {
    QMatrix4x4 rotMat;
    if (this->is_orthographic) {
        // 2Dモード: 方向に合わせた固定回転を適用
        applyDirectionalRotation(rotMat, this->orthographic_direction);
    } else {
        // 3Dモード: オイラー角から回転を生成
        rotMat.rotate(this->rotation_euler.x(), 1, 0, 0); // Pitch
        rotMat.rotate(this->rotation_euler.y(), 0, 1, 0); // Yaw
        rotMat.rotate(this->rotation_euler.z(), 0, 0, 1); // Roll
    } 
    QMatrix4x4 transMat;
    // 左手系にする
    transMat.scale(1.0f, 1.0f, -1.0f);
    transMat.translate(-this->position);

    return rotMat * transMat;
}

void applyDirectionalRotation(QMatrix4x4 &mat, Direction direction) {
    switch (direction) {
    default:
    case Direction::Front:
        break;
    case Direction::Back:
        mat.rotate(180, 0, 1, 0);
        break;
    case Direction::Top:
        mat.rotate(90, 1, 0, 0);
        break;
    case Direction::Bottom:
        mat.rotate(-90, 1, 0, 0);
        break;
    case Direction::Left:
        mat.rotate(90, 0, 1, 0);
        break;
    case Direction::Right:
        mat.rotate(-90, 0, 1, 0);
        break;
    }
}