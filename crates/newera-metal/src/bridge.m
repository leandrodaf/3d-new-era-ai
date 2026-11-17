#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef struct {
    float eye[4], forward[4], right[4], up[4], sky[4], sun_direction[4], sun_color[4];
    uint32_t width, height, first_row, rows, samples, bounces, opaque_triangles, light_count;
    float squeeze;
    uint32_t coarse, padding[2];
} PhotoParams;

@interface NewEraMetalScene : NSObject
@property(nonatomic, strong) id<MTLDevice> device;
@property(nonatomic, strong) id<MTLCommandQueue> queue;
@property(nonatomic, strong) id<MTLComputePipelineState> pipeline;
@property(nonatomic, strong) id<MTLAccelerationStructure> acceleration;
@property(nonatomic, strong) id<MTLBuffer> vertices;
@property(nonatomic, strong) id<MTLBuffer> indices;
@property(nonatomic, strong) id<MTLBuffer> lights;
@property(nonatomic, strong) id<MTLTexture> textures;
@property(nonatomic, strong) id<MTLBuffer> output;
@end
@implementation NewEraMetalScene
@end

static id<MTLComputePipelineState> photoPipeline(id<MTLDevice> device,
                                                const uint8_t *source, size_t length) API_AVAILABLE(macos(12.0)) {
    // This bridge receives one shader, embedded in the application binary.
    static id<MTLComputePipelineState> pipeline;
    @synchronized ([NewEraMetalScene class]) {
        if (pipeline && pipeline.device == device) return pipeline;
        NSString *shader = [[NSString alloc] initWithBytes:source length:length
                                                  encoding:NSUTF8StringEncoding];
        if (!shader) return nil;
        MTLCompileOptions *options = [MTLCompileOptions new];
        options.languageVersion = MTLLanguageVersion2_4;
        options.fastMathEnabled = NO;
        NSError *error = nil;
        id<MTLLibrary> library = [device newLibraryWithSource:shader options:options error:&error];
        id<MTLFunction> function = [library newFunctionWithName:@"photo"];
        pipeline = function ? [device newComputePipelineStateWithFunction:function error:&error] : nil;
        if (!pipeline && error) {
            fprintf(stderr, "New Era Metal shader: %s\n", error.localizedDescription.UTF8String);
        }
        return pipeline;
    }
}

void *newera_metal_create(const uint8_t *source, size_t source_length,
                         const uint8_t *vertices, size_t vertex_bytes,
                         const uint32_t *indices, size_t index_count,
                         const uint8_t *textures, size_t layers,
                         const float *lights, size_t light_count) {
    if (@available(macOS 12.0, *)) {
        @autoreleasepool {
            id<MTLDevice> device = MTLCreateSystemDefaultDevice();
            if (!device.supportsRaytracing || layers > 2048 ||
                vertex_bytes > device.maxBufferLength || index_count > device.maxBufferLength / 4 ||
                light_count > device.maxBufferLength / 64) return NULL;
            NewEraMetalScene *scene = [NewEraMetalScene new];
            scene.device = device;
            scene.pipeline = photoPipeline(device, source, source_length);
            scene.queue = [device newCommandQueue];
            if (!scene.pipeline || !scene.queue) return NULL;
            scene.vertices = [device newBufferWithBytes:vertices length:vertex_bytes
                                                options:MTLResourceStorageModeShared];
            scene.indices = [device newBufferWithBytes:indices length:index_count * sizeof(uint32_t)
                                               options:MTLResourceStorageModeShared];
            scene.lights = light_count
                ? [device newBufferWithBytes:lights length:light_count * 64
                                     options:MTLResourceStorageModeShared]
                : [device newBufferWithLength:64 options:MTLResourceStorageModeShared];
            MTLTextureDescriptor *texture = [MTLTextureDescriptor new];
            texture.textureType = MTLTextureType2DArray;
            texture.pixelFormat = MTLPixelFormatRGBA8Unorm;
            texture.width = texture.height = 256;
            texture.arrayLength = MAX(layers, 1);
            texture.usage = MTLTextureUsageShaderRead;
            texture.storageMode = MTLStorageModeShared;
            scene.textures = [device newTextureWithDescriptor:texture];
            if (!scene.vertices || !scene.indices || !scene.lights || !scene.textures) return NULL;
            for (NSUInteger layer = 0; layer < layers; layer++) {
                [scene.textures replaceRegion:MTLRegionMake2D(0, 0, 256, 256)
                                  mipmapLevel:0 slice:layer withBytes:textures + layer * 256 * 1024
                                  bytesPerRow:1024 bytesPerImage:256 * 1024];
            }
            MTLAccelerationStructureTriangleGeometryDescriptor *geometry =
                [MTLAccelerationStructureTriangleGeometryDescriptor descriptor];
            geometry.vertexBuffer = scene.vertices;
            geometry.vertexStride = 52;
            geometry.indexBuffer = scene.indices;
            geometry.indexType = MTLIndexTypeUInt32;
            geometry.triangleCount = index_count / 3;
            geometry.opaque = YES;
            MTLPrimitiveAccelerationStructureDescriptor *desc =
                [MTLPrimitiveAccelerationStructureDescriptor descriptor];
            desc.geometryDescriptors = @[geometry];
            MTLAccelerationStructureSizes sizes = [device accelerationStructureSizesWithDescriptor:desc];
            scene.acceleration = [device newAccelerationStructureWithSize:sizes.accelerationStructureSize];
            id<MTLBuffer> scratch = [device newBufferWithLength:MAX(sizes.buildScratchBufferSize, 16)
                                                       options:MTLResourceStorageModePrivate];
            id<MTLCommandBuffer> command = [scene.queue commandBuffer];
            id<MTLAccelerationStructureCommandEncoder> encoder = [command accelerationStructureCommandEncoder];
            if (!scene.acceleration || !scratch || !encoder) return NULL;
            [encoder buildAccelerationStructure:scene.acceleration descriptor:desc
                                  scratchBuffer:scratch scratchBufferOffset:0];
            [encoder endEncoding];
            [command commit];
            [command waitUntilCompleted];
            if (command.status != MTLCommandBufferStatusCompleted) return NULL;
            return (__bridge_retained void *)scene;
        }
    }
    return NULL;
}

bool newera_metal_render(void *handle, const PhotoParams *params, float *pixels) {
    if (@available(macOS 12.0, *)) {
        @autoreleasepool {
            NewEraMetalScene *scene = (__bridge NewEraMetalScene *)handle;
            size_t count = (size_t)params->width * params->rows;
            if (count > scene.device.maxBufferLength / 48) return false;
            size_t bytes = count * 48;
            if (scene.output.length < bytes) {
                scene.output = [scene.device newBufferWithLength:bytes options:MTLResourceStorageModeShared];
            }
            id<MTLCommandBuffer> command = [scene.queue commandBuffer];
            id<MTLComputeCommandEncoder> encoder = [command computeCommandEncoder];
            if (!scene.output || !encoder) return false;
            [encoder setComputePipelineState:scene.pipeline];
            [encoder setAccelerationStructure:scene.acceleration atBufferIndex:0];
            [encoder setBuffer:scene.vertices offset:0 atIndex:1];
            [encoder setBuffer:scene.indices offset:0 atIndex:2];
            [encoder setBuffer:scene.lights offset:0 atIndex:3];
            [encoder setBytes:params length:sizeof(*params) atIndex:4];
            [encoder setBuffer:scene.output offset:0 atIndex:5];
            [encoder setTexture:scene.textures atIndex:0];
            [encoder dispatchThreads:MTLSizeMake(count, 1, 1)
               threadsPerThreadgroup:MTLSizeMake(scene.pipeline.threadExecutionWidth, 1, 1)];
            [encoder endEncoding];
            [command commit];
            [command waitUntilCompleted];
            if (command.status != MTLCommandBufferStatusCompleted) return false;
            memcpy(pixels, scene.output.contents, bytes);
            return true;
        }
    }
    return false;
}

void newera_metal_destroy(void *handle) {
    @autoreleasepool {
        NewEraMetalScene *scene = (__bridge_transfer NewEraMetalScene *)handle;
        (void)scene;
    }
}
