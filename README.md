# Prerequisites
- Have permission to both uinput and /dev/input/eventX 
    - For this just create a new input group and put yourself as part of it via rules.d 
    - Will add a section for how to do this later
- Also for eventX it's the specific keyboard device you're tryna capture the events of
# Things to Add
- Mouse macro?
- Setup script to avoid the hassle of tweaking files
# Dumb Bugs
- sudo udevadm info /dev/uinput to check if devices are running
- if it gives unknown device do the stuff below
- Uinput might not be running(check with lsmod | grep uinput)
- sudo modprove uinput should fix it
- 