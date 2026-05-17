#include <QMatrix4x4>
#include <QVector3D>

enum class Direction;

class Camera {
    QVector3D position;
    QVector3D rotation_euler;
    Direction orthographic_direction;
    bool is_orthographic = false;

  public:
    QMatrix4x4 getViewMat();
};

enum class Direction { Front, Back, Top, Bottom, Left, Right };