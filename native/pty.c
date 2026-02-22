#define _XOPEN_SOURCE 600
#include <pty.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <sys/wait.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>

// Spawn a new PTY with the given shell.
// Returns master FD on success, -1 on failure.
// Writes child PID to *out_pid.
int pty_spawn(const char *shell, const char *cwd,
              int cols, int rows, int *out_pid) {
    struct winsize ws;
    memset(&ws, 0, sizeof(ws));
    ws.ws_col = cols;
    ws.ws_row = rows;

    int master;
    pid_t pid = forkpty(&master, NULL, NULL, &ws);
    if (pid < 0) return -1;

    if (pid == 0) {
        // Child process
        if (cwd && cwd[0]) chdir(cwd);
        setenv("TERM", "xterm-256color", 1);
        setenv("COLORTERM", "truecolor", 1);
        execlp(shell, shell, (char *)NULL);
        _exit(127);
    }

    // Parent: set master FD to non-blocking
    int flags = fcntl(master, F_GETFL, 0);
    if (flags >= 0) fcntl(master, F_SETFL, flags | O_NONBLOCK);

    *out_pid = (int)pid;
    return master;
}

// Resize the PTY window.
int pty_resize(int fd, int cols, int rows) {
    struct winsize ws;
    memset(&ws, 0, sizeof(ws));
    ws.ws_col = cols;
    ws.ws_row = rows;
    return ioctl(fd, TIOCSWINSZ, &ws);
}

// Non-blocking read from master FD.
// Returns bytes read, 0 if nothing available, -1 on real error/EOF.
int pty_read(int fd, char *buf, int len) {
    ssize_t n = read(fd, buf, len);
    if (n > 0) return (int)n;
    if (n == 0) return -1; // EOF
    if (errno == EAGAIN || errno == EWOULDBLOCK) return 0;
    return -1;
}

// Write to master FD.
int pty_write(int fd, const char *buf, int len) {
    ssize_t n = write(fd, buf, len);
    return (int)n;
}

// Close master FD.
int pty_close(int fd) {
    return close(fd);
}

// Send signal to child.
int pty_kill(int pid, int sig) {
    return kill((pid_t)pid, sig);
}

// Non-blocking waitpid. Returns:
//   0 = still running
//   1 = exited (exit code written to *out_code)
//  -1 = error
int pty_wait(int pid, int *out_code) {
    int status;
    pid_t r = waitpid((pid_t)pid, &status, WNOHANG);
    if (r == 0) return 0;
    if (r < 0) return -1;
    if (WIFEXITED(status)) {
        *out_code = WEXITSTATUS(status);
        return 1;
    }
    if (WIFSIGNALED(status)) {
        *out_code = 128 + WTERMSIG(status);
        return 1;
    }
    return -1;
}
