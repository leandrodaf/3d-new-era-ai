#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

@interface NewEraVideo : NSObject
@property(nonatomic, strong) id<MTLCommandQueue> queue;
@property(nonatomic, strong) id<MTLRenderPipelineState> opaque, glass;
@property(nonatomic, strong) id<MTLComputePipelineState> outline, resolve;
@property(nonatomic, strong) id<MTLDepthStencilState> depthWrite, depthRead;
@property(nonatomic, strong) id<MTLBuffer> vertices, indices, transparent, output;
@property(nonatomic, strong) id<MTLTexture> images, color, normals, outlined, depth;
@end
@implementation NewEraVideo
@end

static id<MTLRenderPipelineState> makePipeline(id<MTLDevice> device, id<MTLLibrary> library, bool glass) {
    MTLRenderPipelineDescriptor *desc = [MTLRenderPipelineDescriptor new];
    desc.vertexFunction = [library newFunctionWithName:@"video_vertex"];
    desc.fragmentFunction = [library newFunctionWithName:glass ? @"video_glass" : @"video_opaque"];
    desc.colorAttachments[0].pixelFormat = MTLPixelFormatRGBA16Float;
    desc.depthAttachmentPixelFormat = MTLPixelFormatDepth32Float;
    if (glass) {
        desc.colorAttachments[0].blendingEnabled = YES;
        desc.colorAttachments[0].sourceRGBBlendFactor = MTLBlendFactorSourceAlpha;
        desc.colorAttachments[0].destinationRGBBlendFactor = MTLBlendFactorOneMinusSourceAlpha;
    } else {
        desc.colorAttachments[1].pixelFormat = MTLPixelFormatRGBA16Float;
    }
    NSError *error = nil;
    id<MTLRenderPipelineState> pipeline = [device newRenderPipelineStateWithDescriptor:desc error:&error];
    if (!pipeline && error) fprintf(stderr, "New Era Metal video: %s\n", error.localizedDescription.UTF8String);
    return pipeline;
}

static id<MTLTexture> target(id<MTLDevice> device, MTLPixelFormat format, size_t w, size_t h, MTLTextureUsage usage) {
    MTLTextureDescriptor *desc = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:format
                                                                                 width:w height:h mipmapped:NO];
    desc.storageMode = MTLStorageModePrivate;
    desc.usage = usage;
    return [device newTextureWithDescriptor:desc];
}

void *newera_video_create(const uint8_t *source, size_t source_len,
                         const uint8_t *vertices, size_t vertex_bytes, const uint32_t *indices,
                         size_t count, size_t clear_count, const uint8_t *images, size_t layers,
                         size_t width, size_t height) {
    if (@available(macOS 12.0, *)) {
        @autoreleasepool {
            id<MTLDevice> device = MTLCreateSystemDefaultDevice();
            if (!device || layers > 2048 || vertex_bytes > device.maxBufferLength ||
                count > device.maxBufferLength/4 || clear_count > device.maxBufferLength/4) return NULL;
            NSString *shader = [[NSString alloc] initWithBytes:source length:source_len encoding:NSUTF8StringEncoding];
            if (!shader) return NULL;
            MTLCompileOptions *options = [MTLCompileOptions new];
            options.languageVersion = MTLLanguageVersion2_4;
            NSError *error = nil;
            id<MTLLibrary> library = [device newLibraryWithSource:shader options:options error:&error];
            if (!library) {
                if (error) fprintf(stderr, "New Era Metal video shader: %s\n", error.localizedDescription.UTF8String);
                return NULL;
            }
            NewEraVideo *video = [NewEraVideo new];
            video.queue = [device newCommandQueue];
            video.opaque = makePipeline(device, library, false);
            video.glass = makePipeline(device, library, true);
            video.outline = [device newComputePipelineStateWithFunction:[library newFunctionWithName:@"video_outline"] error:&error];
            video.resolve = [device newComputePipelineStateWithFunction:[library newFunctionWithName:@"video_resolve"] error:&error];
            if (!video.queue || !video.opaque || !video.glass || !video.outline || !video.resolve) return NULL;
            MTLDepthStencilDescriptor *depth = [MTLDepthStencilDescriptor new];
            depth.depthCompareFunction = MTLCompareFunctionLess;
            depth.depthWriteEnabled = YES;
            video.depthWrite = [device newDepthStencilStateWithDescriptor:depth];
            depth.depthWriteEnabled = NO;
            video.depthRead = [device newDepthStencilStateWithDescriptor:depth];
            video.vertices = [device newBufferWithBytes:vertices length:vertex_bytes options:MTLResourceStorageModeShared];
            video.indices = [device newBufferWithBytes:indices length:count*4 options:MTLResourceStorageModeShared];
            video.transparent = [device newBufferWithLength:MAX(4, clear_count*4) options:MTLResourceStorageModeShared];
            video.output = [device newBufferWithLength:width*height*4 options:MTLResourceStorageModeShared];
            video.depth = target(device,MTLPixelFormatDepth32Float,width*2,height*2,MTLTextureUsageRenderTarget);
            MTLTextureUsage read_render = MTLTextureUsageRenderTarget | MTLTextureUsageShaderRead;
            video.color = target(device,MTLPixelFormatRGBA16Float,width*2,height*2,read_render);
            video.normals = target(device,MTLPixelFormatRGBA16Float,width*2,height*2,read_render);
            video.outlined = target(device,MTLPixelFormatRGBA16Float,width*2,height*2,read_render | MTLTextureUsageShaderWrite);

            MTLTextureDescriptor *tex = [MTLTextureDescriptor new];
            tex.textureType = MTLTextureType2DArray;
            tex.pixelFormat = MTLPixelFormatRGBA8Unorm;
            tex.width = tex.height = 256;
            tex.arrayLength = MAX(layers,1);
            tex.storageMode = MTLStorageModeShared;
            tex.usage = MTLTextureUsageShaderRead;
            video.images = [device newTextureWithDescriptor:tex];
            if (!video.depthWrite || !video.depthRead || !video.vertices || !video.indices ||
                !video.transparent || !video.output || !video.depth || !video.color ||
                !video.normals || !video.outlined || !video.images) return NULL;
            for (size_t layer = 0; layer < layers; layer++) {
                [video.images replaceRegion:MTLRegionMake2D(0,0,256,256) mipmapLevel:0 slice:layer
                                  withBytes:images+layer*256*1024 bytesPerRow:1024 bytesPerImage:256*1024];
            }
            return (__bridge_retained void *)video;
        }
    }
    return NULL;
}

static bool draw(id<MTLCommandBuffer> command, NewEraVideo *video, const float *view,
                 const float *sky, size_t count, bool glass) {
    MTLRenderPassDescriptor *pass = [MTLRenderPassDescriptor renderPassDescriptor];
    pass.colorAttachments[0].texture = glass ? video.outlined : video.color;
    pass.colorAttachments[0].loadAction = glass ? MTLLoadActionLoad : MTLLoadActionClear;
    pass.colorAttachments[0].storeAction = MTLStoreActionStore;
    pass.colorAttachments[0].clearColor = MTLClearColorMake(sky[0],sky[1],sky[2],1);
    if (!glass) {
        pass.colorAttachments[1].texture = video.normals;
        pass.colorAttachments[1].loadAction = MTLLoadActionClear;
        pass.colorAttachments[1].storeAction = MTLStoreActionStore;
        pass.colorAttachments[1].clearColor = MTLClearColorMake(0,0,0,0);
    }
    pass.depthAttachment.texture = video.depth;
    pass.depthAttachment.loadAction = glass ? MTLLoadActionLoad : MTLLoadActionClear;
    pass.depthAttachment.storeAction = glass ? MTLStoreActionDontCare : MTLStoreActionStore;
    pass.depthAttachment.clearDepth = 1;
    id<MTLRenderCommandEncoder> encoder = [command renderCommandEncoderWithDescriptor:pass];
    if (!encoder) return false;
    [encoder setRenderPipelineState:glass ? video.glass : video.opaque];
    [encoder setDepthStencilState:glass ? video.depthRead : video.depthWrite];
    [encoder setFrontFacingWinding:MTLWindingCounterClockwise];
    [encoder setCullMode:glass ? MTLCullModeNone : MTLCullModeBack];
    [encoder setVertexBuffer:video.vertices offset:0 atIndex:0];
    [encoder setVertexBytes:view length:64 atIndex:1];
    [encoder setFragmentTexture:video.images atIndex:0];
    [encoder drawIndexedPrimitives:MTLPrimitiveTypeTriangle indexCount:count indexType:MTLIndexTypeUInt32
                         indexBuffer:glass ? video.transparent : video.indices indexBufferOffset:0];
    [encoder endEncoding];
    return true;
}

bool newera_video_render(void *handle, const float *view, const float *sky, const uint32_t *clear,
                         size_t count, size_t clear_count, uint8_t *pixels) {
    @autoreleasepool {
        NewEraVideo *video = (__bridge NewEraVideo *)handle;
        if (clear_count) memcpy(video.transparent.contents,clear,clear_count*4);
        id<MTLCommandBuffer> command = [video.queue commandBuffer];
        if (!draw(command, video, view, sky, count, false)) return false;
        id<MTLComputeCommandEncoder> encoder = [command computeCommandEncoder];
        if (!encoder) return false;
        size_t w = video.color.width, h = video.color.height;
        [encoder setComputePipelineState:video.outline];
        [encoder setTexture:video.color atIndex:0];
        [encoder setTexture:video.normals atIndex:1];
        [encoder setTexture:video.outlined atIndex:2];
        [encoder dispatchThreads:MTLSizeMake(w,h,1) threadsPerThreadgroup:MTLSizeMake(8,8,1)];
        [encoder endEncoding];
        if (clear_count && !draw(command,video,view,sky,clear_count,true)) return false;
        encoder = [command computeCommandEncoder];
        if (!encoder) return false;
        [encoder setComputePipelineState:video.resolve];
        [encoder setTexture:video.outlined atIndex:0];
        [encoder setBuffer:video.output offset:0 atIndex:0];
        [encoder dispatchThreads:MTLSizeMake(w/2,h/2,1) threadsPerThreadgroup:MTLSizeMake(8,8,1)];
        [encoder endEncoding];
        [command commit];
        [command waitUntilCompleted];
        if (command.status != MTLCommandBufferStatusCompleted) return false;
        memcpy(pixels,video.output.contents,video.output.length);
        return true;
    }
}

void newera_video_destroy(void *handle) {
    @autoreleasepool {
        NewEraVideo *video = (__bridge_transfer NewEraVideo *)handle;
        (void)video;
    }
}
