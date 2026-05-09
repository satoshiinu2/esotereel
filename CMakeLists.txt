cmake_minimum_required(VERSION 3.16)
project(EsotereelGui)
set(QT_NO_PRIVATE_MODULE_WARNING ON CACHE BOOL "" FORCE)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTOUIC ON)
set(CMAKE_AUTORCC ON)

# Allow setting Qt6 path via environment variable
if(NOT Qt6_DIR AND DEFINED ENV{QT_DIR})
    list(APPEND CMAKE_PREFIX_PATH "$ENV{QT_DIR}")
endif()

# ビルドタイプに応じた設定の切り替え
if(NOT CMAKE_BUILD_TYPE OR CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(IS_DEBUG TRUE)
else()
    set(IS_DEBUG FALSE)
endif()

if(IS_DEBUG)
    set(RUST_PROFILE "debug")
    set(RUST_FLAG "")
else()
    set(RUST_PROFILE "release")
    set(RUST_FLAG "--release")
endif()

# ライブラリの拡張子とか
if(WIN32)
    set(DLIB_EXT "dll")
    set(RLIB_PREFIX "")
elseif(APPLE)
    set(DLIB_EXT "dylib")
    set(RLIB_PREFIX "lib")
else()
    set(DLIB_EXT "so")
    set(RLIB_PREFIX "lib")
endif()

# Qt6 components - Wayland is platform-specific
set(QT_COMPONENTS Widgets Network Gui Core)
if(UNIX AND NOT APPLE)
    list(APPEND QT_COMPONENTS WaylandClient)
endif()

find_package(Qt6 REQUIRED COMPONENTS ${QT_COMPONENTS})

# qt ads
include(FetchContent)
FetchContent_Declare(
    ads
    GIT_REPOSITORY https://github.com/githubuser0xFFFF/Qt-Advanced-Docking-System.git
    GIT_TAG        4.4.1
)
set(BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)  
set(ADS_VERSION "4.4.1" CACHE STRING "" FORCE)  
FetchContent_MakeAvailable(ads)

# ADSターゲットが存在する場合、そのターゲットに対する警告をすべて無効化する
if (TARGET qtadvanceddocking-qt6)
    if(NOT MSVC)
        # GCC/Clang用: すべての警告を抑制 (-w)
        target_compile_options(qtadvanceddocking-qt6 PRIVATE "-w")
    else()
        # MSVC用
        target_compile_options(qtadvanceddocking-qt6 PRIVATE "/W0")
    endif()
endif()

# ライブラリとコア
set(RUST_TARGET_DIR "${CMAKE_SOURCE_DIR}/target/${RUST_PROFILE}")
set(GUI_HLP_LIB "${RUST_TARGET_DIR}/${RLIB_PREFIX}esotereel_gui_helper.${DLIB_EXT}")
set(CORE_EXE "${RUST_TARGET_DIR}/esotereel_core")
file(GLOB_RECURSE RUST_SRC_FILES
    "${CMAKE_SOURCE_DIR}/core/src/*.rs"
    "${CMAKE_SOURCE_DIR}/lib/src/*.rs"
    "${CMAKE_SOURCE_DIR}/guihlp/src/*.rs"
    "${CMAKE_SOURCE_DIR}/Cargo.toml"
)

# rustビルドするやつ
add_custom_command(
    OUTPUT ${GUI_HLP_LIB} ${CORE_EXE}
    COMMAND cargo build ${RUST_FLAG}
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    DEPENDS ${RUST_SRC_FILES}
    USES_TERMINAL 
    COMMENT "Building Rust Workspace (${RUST_PROFILE})..."
)

add_custom_target(rust_workspace ALL DEPENDS ${GUI_HLP_LIB} ${CORE_EXE})


# 実行ファイル
file(GLOB_RECURSE SRC_FILES CONFIGURE_DEPENDS gui/src/*.cpp)
add_executable(esotereel_gui ${SRC_FILES})


# dll deploy
if(WIN32)
    if(IS_DEBUG)
        set(DLL_DEPLOY_CMD "powershell -ExecutionPolicy Bypass -File ./deploydebug.ps1")
    else()
        set(DLL_DEPLOY_CMD "powershell -ExecutionPolicy Bypass -File ./deploydist.ps1")
    endif()

    add_custom_command(TARGET esotereel_gui POST_BUILD
        COMMAND ${DLL_DEPLOY_CMD}
        WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    )
endif()

# # ポート使ってるやつ消す
# if(UNIX AND NOT APPLE)
#     add_custom_target(cleanup_port COMMAND fuser -k 12345/tcp || true)
# else()
#     add_custom_target(cleanup_port)
# endif()

# # 依存関係
# add_dependencies(esotereel_gui rust_workspace cleanup_port)

# ライブラリリンク
target_include_directories(esotereel_gui PRIVATE "${CMAKE_SOURCE_DIR}/guihlp/include")
target_link_libraries(esotereel_gui PRIVATE 
    Qt6::Widgets 
    Qt6::Network
    qtadvanceddocking-qt6 
    ${GUI_HLP_LIB}
)

if(TARGET Qt6::WaylandClient)
    target_link_libraries(esotereel_gui PRIVATE Qt6::WaylandClient Qt6::WaylandClientPrivate)
endif()

if (CMAKE_BUILD_TYPE STREQUAL "Debug")
    target_compile_options(esotereel_gui PRIVATE -fsanitize=address -fno-omit-frame-pointer)
    target_link_options(esotereel_gui PRIVATE -fsanitize=address)
endif()
