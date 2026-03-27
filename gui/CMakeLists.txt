cmake_minimum_required(VERSION 3.16)
project(VideoEditorGUI)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTOUIC ON)
set(CMAKE_AUTORCC ON)

# ビルドタイプに応じた設定の切り替え
if(CMAKE_BUILD_TYPE STREQUAL "Debug" OR NOT CMAKE_BUILD_TYPE)
    set(RUST_PROFILE "debug")
    set(RUST_FLAG "")
else()
    set(RUST_PROFILE "release")
    set(RUST_FLAG "--release")
endif()


# Qt6
find_package(Qt6 REQUIRED COMPONENTS Widgets Network)  

# qt ads
include(FetchContent)
FetchContent_Declare(
    ads
    GIT_REPOSITORY https://github.com/githubuser0xFFFF/Qt-Advanced-Docking-System.git
    GIT_TAG        4.3.1
)
set(BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)  
set(ADS_VERSION "4.3.1" CACHE STRING "" FORCE)  
FetchContent_MakeAvailable(ads)

# ライブラリとコア
set(RUST_TARGET_DIR "${CMAKE_SOURCE_DIR}/../target/${RUST_PROFILE}")
set(GUI_HLP_LIB "${RUST_TARGET_DIR}/libnomyoedit_gui_helper.so")
set(CORE_EXE "${RUST_TARGET_DIR}/nomyoedit_core")

# ビルドするやつ
add_custom_command(
    OUTPUT ${GUI_HLP_LIB} ${CORE_EXE}
    COMMAND cargo build ${RUST_FLAG}
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}/..
    COMMENT "Building Rust Workspace (${RUST_PROFILE})..."
)

add_custom_target(rust_workspace ALL DEPENDS ${GUI_HLP_LIB} ${CORE_EXE})


# 実行ファイル
file(GLOB_RECURSE SRC_FILES CONFIGURE_DEPENDS src/*.cpp)
add_executable(gui ${SRC_FILES})

# ポート使ってるやつ消す
add_custom_target(cleanup_port COMMAND fuser -k 12345/tcp || true)

# 依存関係
add_dependencies(gui rust_workspace cleanup_port)

# ライブラリリンク
target_include_directories(gui PRIVATE "${CMAKE_SOURCE_DIR}/../guihlp/include")
target_link_libraries(gui PRIVATE 
    Qt6::Widgets 
    Qt6::Network 
    Qt6::Gui       
    Qt6::GuiPrivate
    qt6advanceddocking 
    ${GUI_HLP_LIB}
)