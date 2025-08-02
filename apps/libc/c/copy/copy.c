/* Modified from glibc: io/tst-copy_file_range.c for oscomp.

   Tests for copy_file_range.
   Copyright (C) 2017-2023 Free Software Foundation, Inc.
   This file is part of the GNU C Library.

   The GNU C Library is free software; you can redistribute it and/or
   modify it under the terms of the GNU Lesser General Public
   License as published by the Free Software Foundation; either
   version 2.1 of the License, or (at your option) any later version.

   The GNU C Library is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Lesser General Public License for more details.

   You should have received a copy of the GNU Lesser General Public
   License along with the GNU C Library; if not, see
   <https://www.gnu.org/licenses/ Licenses&#xA;- GNU Project - Free Software Foundation Licenses&#xA;- GNU Project - Free Software Foundation  >.  */

   #define _GNU_SOURCE

   #include <fcntl.h>
   #include <inttypes.h>
   #include <stdarg.h>
   #include <stdbool.h>
   #include <stdio.h>
   #include <stdlib.h>
   #include <string.h>
   #include <sys/stat.h>
   #include <unistd.h>
   
   #ifndef CASE
   #define CASE 1
   #endif
   
   /* array_length (VAR) is the number of elements in the array VAR.  VAR
      must evaluate to an array, not a pointer.  */
   #define array_length(var)                                                   \
     (sizeof(var) / sizeof((var)[0]) +                                         \
      0 * sizeof(struct {                                                      \
        _Static_assert(                                                        \
            !__builtin_types_compatible_p(__typeof(var), __typeof(&(var)[0])), \
            "argument must be an array");                                      \
      }))
   
   /* array_end (VAR) is a pointer one past the end of the array VAR.
      VAR must evaluate to an array, not a pointer.  */
   #define array_end(var) (&(var)[array_length(var)])
   
   void fail_exit(const char* format, ...) {
     va_list ap;
     va_start(ap, format);
     vfprintf(stderr, format, ap);
     va_end(ap);
     exit(EXIT_FAILURE);
   }
   
   #define die(fmt_, ...) \
     fail_exit("%s:%d: " fmt_ "\n", __FILE__, __LINE__, ##__VA_ARGS__)
   
   #define TEST_COMPARE(a_, b_) \
     {                          \
       if (a_ != b_) {          \
         die("compare failed"); \
       }                        \
     }
   
   #define TEST_VERIFY(a_)     \
     {                         \
       if (!a_) {              \
         die("verify failed"); \
       }                       \
     }
   
   void xwrite(int fd, const void* buffer, size_t length) {
     const char* p = buffer;
     const char* end = p + length;
     while (p < end) {
       ssize_t ret = write(fd, p, end - p);
       if (ret < 0)
         die("write of %zu bytes failed after %td", length,
             p - (const char*)buffer);
       if (ret == 0)
         die("write return 0 after writing %td bytes of %zu",
             p - (const char*)buffer, length);
       p += ret;
     }
   }
   
   long long xlseek(int fd, long long offset, int whence) {
     long long result = lseek(fd, offset, whence);
     if (result < 0)
       die("lseek(%d, %lld, %d)", fd, offset, whence);
     return result;
   }
   
   void* xmalloc(size_t n) {
     void* p;
     p = malloc(n);
     if (p == NULL)
       die("malloc %zu", n);
     return p;
   }
   
   int xopen(const char* path, int flags, mode_t mode) {
     int ret = open(path, flags, mode);
     if (ret < 0)
       die("open (\"%s\", 0x%x, 0%o)", path, flags, mode);
     return ret;
   }
   
   void xfstat(int fd, struct stat* result) {
     if (fstat(fd, result) != 0)
       die("fstat (%d)", fd);
   }
   
   void xftruncate(int fd, long long length) {
     if (ftruncate(fd, length) != 0)
       die("ftruncate (%d, %lld)", fd, length);
   }
   
   #define xclose close
   #define test_verbose 0
   
   #ifdef O_LARGEFILE
   #undef O_LARGEFILE
   #endif
   #define O_LARGEFILE 0
   
   int create_temp_file(const char* prefix, char** name) {
     *name = xmalloc(strlen(prefix) + 7); /* +7 for XXXXXX and null terminator */
     strcpy(*name, prefix);
     strcat(*name, "XXXXXX");
     int fd = mkstemp(*name);
     if (fd < 0)
       die("mkstemp: %d", fd);
     printf("created: %s\n", *name);
     return fd;
   }
   
   /* Boolean flags which indicate whether to use pointers with explicit
      output flags.  */
   static int do_inoff;
   static int do_outoff;
   
   /* Name and descriptors of the input files.  Files are truncated and
      reopened (with O_RDWR) between tests.  */
   static char* infile;
   static int infd;
   static char* outfile;
   static int outfd;
   
   /* Input and output offsets.  Set according to do_inoff and do_outoff
      before the test.  The offsets themselves are always set to
      zero.  */
   static off_t inoff;
   static off_t* pinoff;
   static off_t outoff;
   static off_t* poutoff;
   
   /* These are a collection of copy sizes used in tests.    */
   enum { maximum_size = 99999 };
   static const int typical_sizes[] = {0,    1,    2,    3,    1024,        2048,
                                       4096, 8191, 8192, 8193, maximum_size};
   
   /* The random contents of this array can be used as a pattern to check
      for correct write operations.  */
   static unsigned char random_data[maximum_size];
   
   /* The size chosen by the test harness.  */
   static int current_size;
   
   /* Perform a copy of a file.  */
   static void simple_file_copy(void) {
     fprintf(stderr, "\n[DEBUG] %s:%d: --- Entering simple_file_copy ---\n", __FILE__, __LINE__);
     fprintf(stderr, "[DEBUG] %s:%d: Initial current_size = %d\n", __FILE__, __LINE__, current_size);
   
     xwrite(infd, random_data, current_size);
     fprintf(stderr, "[DEBUG] %s:%d: Wrote %d bytes of random_data to infd=%d\n", __FILE__, __LINE__, current_size, infd);
   
     int length;
     int in_skipped; /* Expected skipped bytes in input.  */
     if (do_inoff) {
       xlseek(infd, 1, SEEK_SET);
       inoff = 2;
       length = current_size - 3;
       in_skipped = 2;
       fprintf(stderr, "[DEBUG] %s:%d: do_inoff=true. infd seeked to 1, inoff set to 2.\n", __FILE__, __LINE__);
     } else {
       xlseek(infd, 3, SEEK_SET);
       length = current_size - 5;
       in_skipped = 3;
       fprintf(stderr, "[DEBUG] %s:%d: do_inoff=false. infd seeked to 3.\n", __FILE__, __LINE__);
     }
     int out_skipped; /* Expected skipped bytes before the written data.  */
     if (do_outoff) {
       xlseek(outfd, 4, SEEK_SET);
       outoff = 5;
       out_skipped = 5;
       fprintf(stderr, "[DEBUG] %s:%d: do_outoff=true. outfd seeked to 4, outoff set to 5.\n", __FILE__, __LINE__);
     } else {
       xlseek(outfd, 6, SEEK_SET);
       length = current_size - 6;
       out_skipped = 6;
       fprintf(stderr, "[DEBUG] %s:%d: do_outoff=false. outfd seeked to 6.\n", __FILE__, __LINE__);
     }
     if (length < 0) {
       fprintf(stderr, "[DEBUG] %s:%d: Calculated length %d is negative, setting to 0.\n", __FILE__, __LINE__, length);
       length = 0;
     }
     
     fprintf(stderr, "[DEBUG] %s:%d: Calculated values before copy: length=%d, in_skipped=%d, out_skipped=%d\n", __FILE__, __LINE__, length, in_skipped, out_skipped);
     fprintf(stderr, "[DEBUG] %s:%d: Calling copy_file_range(infd=%d, pinoff=%p, outfd=%d, poutoff=%p, len=%d, flags=0)\n", __FILE__, __LINE__, infd, (void*)pinoff, outfd, (void*)poutoff, length);
     if(pinoff) fprintf(stderr, "[DEBUG] %s:%d: *pinoff (before call) = %lld\n", __FILE__, __LINE__, (long long)inoff);
     if(poutoff) fprintf(stderr, "[DEBUG] %s:%d: *poutoff (before call) = %lld\n", __FILE__, __LINE__, (long long)outoff);
   
     ssize_t copied_bytes = copy_file_range(infd, pinoff, outfd, poutoff, length, 0);
     fprintf(stderr, "[DEBUG] %s:%d: copy_file_range returned %zd\n", __FILE__, __LINE__, copied_bytes);
     TEST_COMPARE(copied_bytes, length);
   
     fprintf(stderr, "[DEBUG] %s:%d: --- Verifying offsets after copy ---\n", __FILE__, __LINE__);
     if (do_inoff) {
       fprintf(stderr, "[DEBUG] %s:%d: Verifying inoff. Expected: %lld, Got: %lld\n", __FILE__, __LINE__, (long long)(2 + length), (long long)inoff);
       TEST_COMPARE(inoff, 2 + length);
       fprintf(stderr, "[DEBUG] %s:%d: Verifying infd file position. Expected: 1, Got: %lld\n", __FILE__, __LINE__, xlseek(infd, 0, SEEK_CUR));
       TEST_COMPARE(xlseek(infd, 0, SEEK_CUR), 1);
     } else {
       fprintf(stderr, "[DEBUG] %s:%d: Verifying infd file position. Expected: %lld, Got: %lld\n", __FILE__, __LINE__, (long long)(3 + length), xlseek(infd, 0, SEEK_CUR));
       TEST_COMPARE(xlseek(infd, 0, SEEK_CUR), 3 + length);
     }
     if (do_outoff) {
       fprintf(stderr, "[DEBUG] %s:%d: Verifying outoff. Expected: %lld, Got: %lld\n", __FILE__, __LINE__, (long long)(5 + length), (long long)outoff);
       TEST_COMPARE(outoff, 5 + length);
       fprintf(stderr, "[DEBUG] %s:%d: Verifying outfd file position. Expected: 4, Got: %lld\n", __FILE__, __LINE__, xlseek(outfd, 0, SEEK_CUR));
       TEST_COMPARE(xlseek(outfd, 0, SEEK_CUR), 4);
     } else {
       fprintf(stderr, "[DEBUG] %s:%d: Verifying outfd file position. Expected: %lld, Got: %lld\n", __FILE__, __LINE__, (long long)(6 + length), xlseek(outfd, 0, SEEK_CUR));
       TEST_COMPARE(xlseek(outfd, 0, SEEK_CUR), 6 + length);
     }
     fprintf(stderr, "[DEBUG] %s:%d: --- Offset verification complete ---\n", __FILE__, __LINE__);
   
     struct stat st;
     xfstat(outfd, &st);
     fprintf(stderr, "[DEBUG] %s:%d: Verifying output file size. st.st_size = %lld\n", __FILE__, __LINE__, (long long)st.st_size);
   
     if (length > 0) {
       TEST_COMPARE(st.st_size, out_skipped + length);
     } else {
       /* If we did not write anything, we also did not add any
          padding.  */
       TEST_COMPARE(st.st_size, 0);
       fprintf(stderr, "[DEBUG] %s:%d: length=0, skipping content verification. --- Exiting simple_file_copy ---\n", __FILE__, __LINE__);
       return;
     }
   
     xlseek(outfd, 0, SEEK_SET);
     char* bytes = xmalloc(st.st_size);
     TEST_COMPARE(read(outfd, bytes, st.st_size), st.st_size);
     fprintf(stderr, "[DEBUG] %s:%d: Verifying output file content...\n", __FILE__, __LINE__);
     for (int i = 0; i < out_skipped; ++i)
       TEST_COMPARE(bytes[i], 0);
     fprintf(stderr, "[DEBUG] %s:%d: Verified %d leading zero bytes in output.\n", __FILE__, __LINE__, out_skipped);
     TEST_VERIFY(memcmp(bytes + out_skipped, random_data + in_skipped, length) ==
                 0);
     fprintf(stderr, "[DEBUG] %s:%d: Verified %d bytes of copied data are correct.\n", __FILE__, __LINE__, length);
     free(bytes);
     fprintf(stderr, "[DEBUG] %s:%d: --- Exiting simple_file_copy ---\n", __FILE__, __LINE__);
   }
   
   /* Test that a short input file results in a shortened copy.  */
   static void short_copy(void) {
     fprintf(stderr, "\n[DEBUG] %s:%d: --- Entering short_copy ---\n", __FILE__, __LINE__);
     if (current_size == 0) {
       fprintf(stderr, "[DEBUG] %s:%d: current_size is 0, skipping test. --- Exiting short_copy ---\n", __FILE__, __LINE__);
       /* Nothing to shorten.  */
       return;
     }
   
     /* Two subtests, one with offset 0 and current_size - 1 bytes, and
        another one with current_size bytes, but offset 1.  */
     for (int shift = 0; shift < 2; ++shift) {
       fprintf(stderr, "[DEBUG] %s:%d: Starting subtest with shift = %d\n", __FILE__, __LINE__, shift);
   
       xftruncate(infd, 0);
       xlseek(infd, 0, SEEK_SET);
       size_t in_size = current_size - !shift;
       xwrite(infd, random_data, in_size);
       fprintf(stderr, "[DEBUG] %s:%d: Wrote %zu bytes to infd, which now has size %zu.\n", __FILE__, __LINE__, in_size, in_size);
   
       if (do_inoff) {
         inoff = shift;
         xlseek(infd, 2, SEEK_SET);
         fprintf(stderr, "[DEBUG] %s:%d: do_inoff=true. inoff=%lld, infd seeked to 2.\n", __FILE__, __LINE__, (long long)inoff);
       } else {
         inoff = 3; /* This is unused if pinoff is NULL, but set for consistency. */
         xlseek(infd, shift, SEEK_SET);
         fprintf(stderr, "[DEBUG] %s:%d: do_inoff=false. infd seeked to %d.\n", __FILE__, __LINE__, shift);
       }
       xftruncate(outfd, 0);
       xlseek(outfd, 0, SEEK_SET);
       outoff = 0;
       fprintf(stderr, "[DEBUG] %s:%d: outfd truncated and seeked to 0. outoff set to 0.\n", __FILE__, __LINE__);
       
       /* First call copies current_size - 1 bytes.  */
       size_t expected_copy_len = current_size - 1;
       fprintf(stderr, "[DEBUG] %s:%d: === First copy_file_range call ===\n", __FILE__, __LINE__);
       fprintf(stderr, "[DEBUG] %s:%d: Calling copy_file_range(len=%d). Expecting to copy %zu bytes.\n", __FILE__, __LINE__, current_size, expected_copy_len);
       if(pinoff) fprintf(stderr, "[DEBUG] %s:%d: *pinoff (before) = %lld\n", __FILE__, __LINE__, (long long)inoff);
       if(poutoff) fprintf(stderr, "[DEBUG] %s:%d: *poutoff (before) = %lld\n", __FILE__, __LINE__, (long long)outoff);
       ssize_t copied_bytes = copy_file_range(infd, pinoff, outfd, poutoff, current_size, 0);
       fprintf(stderr, "[DEBUG] %s:%d: copy_file_range returned %zd\n", __FILE__, __LINE__, copied_bytes);
       TEST_COMPARE(copied_bytes, expected_copy_len);
       
       char* buffer = xmalloc(current_size);
       ssize_t read_bytes = pread(outfd, buffer, current_size, 0);
       fprintf(stderr, "[DEBUG] %s:%d: pread from outfd returned %zd bytes.\n", __FILE__, __LINE__, read_bytes);
       TEST_COMPARE(read_bytes, expected_copy_len);
       fprintf(stderr, "[DEBUG] %s:%d: Verifying copied content against random_data + %d.\n", __FILE__, __LINE__, shift);
       TEST_VERIFY(memcmp(buffer, random_data + shift, expected_copy_len) == 0);
       free(buffer);
   
       fprintf(stderr, "[DEBUG] %s:%d: Verifying offsets after first copy...\n", __FILE__, __LINE__);
       if (do_inoff) {
         TEST_COMPARE(inoff, expected_copy_len + shift);
         TEST_COMPARE(xlseek(infd, 0, SEEK_CUR), 2);
         fprintf(stderr, "[DEBUG] %s:%d: do_inoff=true. inoff=%lld, infd pos=%lld.\n", __FILE__, __LINE__, (long long)inoff, xlseek(infd, 0, SEEK_CUR));
       } else {
         TEST_COMPARE(xlseek(infd, 0, SEEK_CUR), expected_copy_len + shift);
         fprintf(stderr, "[DEBUG] %s:%d: do_inoff=false. infd pos=%lld.\n", __FILE__, __LINE__, xlseek(infd, 0, SEEK_CUR));
       }
       if (do_outoff) {
         TEST_COMPARE(outoff, expected_copy_len);
         TEST_COMPARE(xlseek(outfd, 0, SEEK_CUR), 0);
         fprintf(stderr, "[DEBUG] %s:%d: do_outoff=true. outoff=%lld, outfd pos=%lld.\n", __FILE__, __LINE__, (long long)outoff, xlseek(outfd, 0, SEEK_CUR));
       } else {
         TEST_COMPARE(xlseek(outfd, 0, SEEK_CUR), expected_copy_len);
         fprintf(stderr, "[DEBUG] %s:%d: do_outoff=false. outfd pos=%lld.\n", __FILE__, __LINE__, xlseek(outfd, 0, SEEK_CUR));
       }
   
       /* Second call copies zero bytes.  */
       fprintf(stderr, "[DEBUG] %s:%d: === Second copy_file_range call ===\n", __FILE__, __LINE__);
       fprintf(stderr, "[DEBUG] %s:%d: Calling copy_file_range(len=%d). Expecting to copy 0 bytes.\n", __FILE__, __LINE__, current_size);
       if(pinoff) fprintf(stderr, "[DEBUG] %s:%d: *pinoff (before) = %lld\n", __FILE__, __LINE__, (long long)inoff);
       if(poutoff) fprintf(stderr, "[DEBUG] %s:%d: *poutoff (before) = %lld\n", __FILE__, __LINE__, (long long)outoff);
       copied_bytes = copy_file_range(infd, pinoff, outfd, poutoff, current_size, 0);
       fprintf(stderr, "[DEBUG] %s:%d: copy_file_range returned %zd\n", __FILE__, __LINE__, copied_bytes);
       TEST_COMPARE(copied_bytes, 0);
   
       /* And the offsets are unchanged.  */
       fprintf(stderr, "[DEBUG] %s:%d: Verifying offsets are unchanged after second copy...\n", __FILE__, __LINE__);
       if (do_inoff) {
         TEST_COMPARE(inoff, expected_copy_len + shift);
         TEST_COMPARE(xlseek(infd, 0, SEEK_CUR), 2);
         fprintf(stderr, "[DEBUG] %s:%d: do_inoff=true. inoff=%lld, infd pos=%lld. (Unchanged)\n", __FILE__, __LINE__, (long long)inoff, xlseek(infd, 0, SEEK_CUR));
       } else {
         TEST_COMPARE(xlseek(infd, 0, SEEK_CUR), expected_copy_len + shift);
         fprintf(stderr, "[DEBUG] %s:%d: do_inoff=false. infd pos=%lld. (Unchanged)\n", __FILE__, __LINE__, xlseek(infd, 0, SEEK_CUR));
       }
       if (do_outoff) {
         TEST_COMPARE(outoff, expected_copy_len);
         TEST_COMPARE(xlseek(outfd, 0, SEEK_CUR), 0);
         fprintf(stderr, "[DEBUG] %s:%d: do_outoff=true. outoff=%lld, outfd pos=%lld. (Unchanged)\n", __FILE__, __LINE__, (long long)outoff, xlseek(outfd, 0, SEEK_CUR));
       } else {
         TEST_COMPARE(xlseek(outfd, 0, SEEK_CUR), expected_copy_len);
         fprintf(stderr, "[DEBUG] %s:%d: do_outoff=false. outfd pos=%lld. (Unchanged)\n", __FILE__, __LINE__, xlseek(outfd, 0, SEEK_CUR));
       }
     }
     fprintf(stderr, "[DEBUG] %s:%d: --- Exiting short_copy ---\n", __FILE__, __LINE__);
   }
   
   /* A named test function.  */
   struct test_case {
     const char* name;
     void (*func)(void);
     bool sizes; /* If true, call the test with different current_size values.  */
   };
   
   /* The available test cases.  */
   static struct test_case tests[] = {
       {"simple_file_copy", simple_file_copy, .sizes = true},
       {"short_copy", short_copy, .sizes = true},
   };
   
   void do_test(int n_inoff, int n_outoff, int n_test) {
     fprintf(stderr, "[DEBUG] %s:%d: >>> Starting do_test(n_inoff=%d, n_outoff=%d, n_test=%d) <<<\n", __FILE__, __LINE__, n_inoff, n_outoff, n_test);
     for (unsigned char* p = random_data; p < array_end(random_data); ++p)
       *p = rand() >> 24;
     fprintf(stderr, "[DEBUG] %s:%d: Populated random_data array.\n", __FILE__, __LINE__);
   
     infd = create_temp_file("tst-copy_file_range-in-", &infile);
     outfd = create_temp_file("tst-copy_file_range-out-", &outfile);
     {
       fprintf(stderr, "[DEBUG] %s:%d: Performing probing copy_file_range call.\n", __FILE__, __LINE__);
       ssize_t ret = copy_file_range(infd, NULL, outfd, NULL, 0, 0);
       if (ret != 0) {
         die("copy_file_range probing call: %d", ret);
       }
       fprintf(stderr, "[DEBUG] %s:%d: Probing call successful.\n", __FILE__, __LINE__);
     }
     xclose(infd);
     xclose(outfd);
     fprintf(stderr, "[DEBUG] %s:%d: Closed temporary files after probing.\n", __FILE__, __LINE__);
   
     for (do_inoff = 0; do_inoff < n_inoff; ++do_inoff)
       for (do_outoff = 0; do_outoff < n_outoff; ++do_outoff)
         for (struct test_case* test = tests; test < tests + n_test; ++test)
           for (const int* size = typical_sizes; size < array_end(typical_sizes);
                ++size) {
             current_size = *size;
             fprintf(stderr, "\n[INFO] %s:%d: Starting test: %s, do_inoff=%d, do_outoff=%d, current_size=%d\n",
                     __FILE__, __LINE__, test->name, do_inoff, do_outoff, current_size);
   
             inoff = 0;
             if (do_inoff)
               pinoff = &inoff;
             else
               pinoff = NULL;
             outoff = 0;
             if (do_outoff)
               poutoff = &outoff;
             else
               poutoff = NULL;
             fprintf(stderr, "[DEBUG] %s:%d: Offsets configured: pinoff is %s, poutoff is %s.\n",
                     __FILE__, __LINE__, pinoff ? "set" : "NULL", poutoff ? "set" : "NULL");
   
             infd = xopen(infile, O_RDWR | O_LARGEFILE, 0);
             xftruncate(infd, 0);
             outfd = xopen(outfile, O_RDWR | O_LARGEFILE, 0);
             xftruncate(outfd, 0);
             fprintf(stderr, "[DEBUG] %s:%d: Re-opened and truncated temp files: infd=%d, outfd=%d.\n",
                     __FILE__, __LINE__, infd, outfd);
   
             test->func();
   
             xclose(infd);
             xclose(outfd);
             fprintf(stderr, "[DEBUG] %s:%d: Closed temp files after test run.\n", __FILE__, __LINE__);
   
             if (!test->sizes) {
                fprintf(stderr, "[DEBUG] %s:%d: test->sizes is false, breaking from size loop.\n", __FILE__, __LINE__);
               /* Skip the other sizes unless they have been
                  requested.  */
               break;
             }
           }
   
     fprintf(stderr, "[DEBUG] %s:%d: All test loops finished. Removing temp files.\n", __FILE__, __LINE__);
     remove(infile);
     remove(outfile);
     free(infile);
     free(outfile);
     fprintf(stderr, "[DEBUG] %s:%d: >>> Finished do_test <<<\n", __FILE__, __LINE__);
   }
   
   int main(void) {
   #if CASE == 1
     fprintf(stderr, "[DEBUG] %s:%d: CASE = 1\n", __FILE__, __LINE__);
     do_test(1, 1, 1);
   #elif CASE == 2
     fprintf(stderr, "[DEBUG] %s:%d: CASE = 2\n", __FILE__, __LINE__);
     do_test(1, 1, 2);
   #elif CASE == 3
     fprintf(stderr, "[DEBUG] %s:%d: CASE = 3\n", __FILE__, __LINE__);
     do_test(2, 2, 1);
   #else
     fprintf(stderr, "[DEBUG] %s:%d: CASE = %d (default)\n", __FILE__, __LINE__, CASE);
     do_test(2, 2, 2);
   #endif
     printf("copy-file-range-test: passed case %d\n", CASE);
     return 0;
   }