package wtf.s1.ezlog;

import org.jetbrains.annotations.NotNull;

public class EZLog {

    public static final int VERBOSE = 5;
    public static final int DEBUG = 4;
    public static final int INFO = 3;
    public static final int WARN = 2;
    public static final int ERROR = 1;

    public static final int Aes128Gcm = 1;
    public static final int Aes256Gcm = 2;

    public static final int Compress_Zlib = 1;

    public static final int Compress_Default = 0;
    public static final int Compress_Fast = 1;
    public static final int Compress_Best = 2;

    private static volatile EZLogger defaultLogger;

    public static synchronized void initWith(@NotNull EZLogConfig config) {
        init();
        defaultLogger = new EZLogger(config);
    }

    public static void initNoDefault() {
        init();
    }

    public static void v (String tag, String msg) {
        if (defaultLogger != null) {
            defaultLogger.v(tag, msg);
        }
    }
    public static void d (String tag, String msg) {
        if (defaultLogger != null) {
            defaultLogger.d(tag, msg);
        }
    }
    public static void i (String tag, String msg) {
        if (defaultLogger != null) {
            defaultLogger.i(tag, msg);
        }
    }
    public static void w (String tag, String msg) {
        if (defaultLogger != null) {
            defaultLogger.w(tag, msg);
        }
    }
    public static void e (String tag, String msg) {
        if (defaultLogger != null) {
            defaultLogger.e(tag, msg);
        }
    }

    public static void flush() {
        flushAll();
    }

    /**
     * create log from java
     * @param config log config
     */
    public synchronized static void _createLogger(@NotNull EZLogConfig config) {
        createLogger(config.logName,
                config.maxLevel,
                config.dirPath,
                config.keepDays,
                config.compress,
                config.compressLevel,
                config.cipher,
                config.cipherKey,
                config.cipherNonce
        );
    }

    public static void _log(String logName, int level, String target, String logContent) {
        log(logName, level, target, logContent);
    }

    /**
     * native init log library
     */
    private static synchronized native void init();

    /**
     * native create a logger to print log
     *
     * @param logName       logger's name
     * @param maxLevel      max log out level
     * @param dirPath       log file in dir
     * @param keepDays      log live in days
     * @param compress      compress kind
     * @param compressLevel compress level
     * @param cipher        crypto kind
     * @param cipherKey     crypto key
     * @param cipherNonce   crypto nonce
     */
    private static native void createLogger(
            String logName,
            int maxLevel,
            String dirPath,
            int keepDays,
            int compress,
            int compressLevel,
            int cipher,
            byte[] cipherKey,
            byte[] cipherNonce
    );

    /**
     * native  print log to file, the log is associate by logName, filter by level
     *
     * @param logName    logger name
     * @param level      log level
     * @param target     log target
     * @param logContent log message
     */
    private static native void log(String logName, int level, String target, String logContent);

    /**
     * native flush all logger, sync content to file
     */
    private static native void flushAll();

    static {
        System.loadLibrary("ezlog");
    }
}