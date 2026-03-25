cmake_minimum_required(VERSION 3.16)
project(VideoEditorGUI)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTOUIC ON)
set(CMAKE_AUTORCC ON)

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

# Rust ライブラリパス
set(MUSCL_LIB_PATH "${CMAKE_SOURCE_DIR}/../lib/target/release/libmuscedit_lib.so")
set(MUSCL_LIB_HEADER_PATH "${CMAKE_SOURCE_DIR}/../lib/include")
set(MUSCL_LIB_HEADER "${CMAKE_SOURCE_DIR}/../lib/include/muscedit_lib.h")
set(MUSCL_CORE_PATH "${CMAKE_SOURCE_DIR}/../core/target/release/muscedit-core")

# lib ビルド
add_custom_command(
    OUTPUT ${MUSCL_LIB_PATH}
    COMMAND cargo build --release
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}/../lib
    COMMENT "Building Muscledit library..."
)

add_custom_target(rust_lib ALL DEPENDS ${MUSCL_LIB_PATH})

# core ビルド
add_custom_command(
    OUTPUT ${MUSCL_CORE_PATH}
    COMMAND cargo build --release
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}/../core
    COMMENT "Building Muscledit core..."
)

add_custom_target(rust_core ALL DEPENDS ${MUSCL_CORE_PATH})

# ポート使ってるやつ消す
add_custom_target(cleanup_port
    COMMAND fuser -k 12345/tcp || true
)

# 実行ファイル
file(GLOB_RECURSE SRC_FILES CONFIGURE_DEPENDS
    src/*.cpp
)

add_executable(gui 
    ${SRC_FILES}
    ${SRC_FILES}
    ${PROTO_SRCS}
)

add_dependencies(gui rust_lib rust_core cleanup_port)

set_source_files_properties(${SRC_FILES} PROPERTIES
    OBJECT_DEPENDS ${MUSCL_LIB_HEADER}
)

# ライブラリリンク
target_link_libraries(gui PRIVATE Qt6::Widgets Qt6::Network)
target_link_libraries(gui PRIVATE qt6advanceddocking)
target_link_libraries(gui PRIVATE ${MUSCL_LIB_PATH})

target_include_directories(gui PRIVATE ${MUSCL_LIB_HEADER_PATH})