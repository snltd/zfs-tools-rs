#!/bin/sh
 
for tool in z*
do
  cargo install --path $tool
done

