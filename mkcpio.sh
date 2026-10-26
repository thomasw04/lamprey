#!/bin/bash
(cd initrd && find . -type f -print0 |
    cpio --null -o --format=newc) > initrd.cpio
