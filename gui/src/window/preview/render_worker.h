#include "../../wrapper/project/timeline.h"
#include "../../wrapper/wgpuutil.h"
#include "../main.h"
#include "../timeline/timeline.h"
#include <QObject>

class RenderWorker : public QObject {
    Q_OBJECT
  public:
    explicit RenderWorker(WindowGState *state);

  public slots:
    void render(Timeline timeline, CameraInfo *camera, int64_t frame);

  private:
    WindowGState *windowState;
    std::optional<WGpuUtil> wgpuutil;
};