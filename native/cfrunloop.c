#include <CoreFoundation/CoreFoundation.h>
#include <CoreServices/CoreServices.h>

typedef void (*ExternalTimerCallback)(void*);

static CFRunLoopTimerRef timerRef = NULL;
static ExternalTimerCallback externalCallback = NULL;
static void* externalContext = NULL;
static int remaining = 10;

void timerCallback(CFRunLoopTimerRef timerRef, void* info) {
	externalCallback(externalContext);
}

void cfRunLoopRun(ExternalTimerCallback callback, void* context) {
	externalCallback = callback;
	externalContext = context;

	timerRef = CFRunLoopTimerCreate(NULL, 0, 1.0, 0, 0, timerCallback, NULL);
	CFRunLoopAddTimer(CFRunLoopGetCurrent(), timerRef, kCFRunLoopCommonModes);

	CFRunLoopRun();
}

void cfRunLoopStop() {
	CFRunLoopTimerInvalidate(timerRef);
	CFRunLoopRemoveTimer(CFRunLoopGetCurrent(), timerRef, kCFRunLoopCommonModes);
	CFRunLoopStop(CFRunLoopGetCurrent());
	timerRef = NULL;
}
