// Copyright 2018 Google LLC
// 
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// 
//     https://www.apache.org/licenses/LICENSE-2.0
// 
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

const NS = "http://www.w3.org/2000/svg";

function xy_of(x, y) {
    return (y << 16) | x;
}

function x_of(xy) {
    return xy & 0xffff;
}

function y_of(xy) {
    return xy >> 16;
}

class Ui {
    constructor(el) {
        this.el = el;
        this.wireGrid = new WireGrid(48, 32);
    }

    init() {
        this.circle = this.addCircle(100, 100, 10, '#444');
        var self = this;
        this.el.ondragover = function(ev) {
            ev.preventDefault();
            console.log(ev);
            self.circle.setAttribute('cx', ev.offsetX);
            self.circle.setAttribute('cy', ev.offsetY);
        };

        this.el.onmousedown = function(ev) {
            self.wireGrid.onmousedown(ev);
        }
        this.el.onmouseup = function(ev) {
            self.wireGrid.onmouseup(ev);
        }
        this.el.onmousemove = function(ev) {
            self.wireGrid.onmousemove(ev);
        }
        this.wireGrid.attach(this.el);
    }

    addCircle(x, y, r, color) {
        let circle = document.createElementNS(NS, 'circle');
        circle.setAttribute('cx', x);
        circle.setAttribute('cy', y);
        circle.setAttribute('r', r);
        circle.style.fill = color;
        this.el.appendChild(circle);
        return circle;
    }

}

class WireGrid {
    constructor(w, h) {
        this.w = w;
        this.h = h;
        this.scale = 10;
        this.hset = new Set();
        this.vset = new Set();
        this.hmap = new Map();
        this.vmap = new Map();
        this.clickState = 0;
    }

    attach(parent) {
        this.parent = parent;
        // use translate in DOM?
        this.x0 = 0;
        this.y0 = 0;
        let rect = document.createElementNS(NS, 'rect');
        rect.setAttribute('x', this.x0);
        rect.setAttribute('y', this.y0);
        rect.setAttribute('width', this.w * this.scale);
        rect.setAttribute('height', this.h * this.scale);
        rect.setAttribute('fill', '#cdf');
        parent.appendChild(rect);

        for (let x = 0; x <= this.w; x++) {
            let line = document.createElementNS(NS, 'line');
            line.setAttribute('x1', this.x0 + x * this.scale);
            line.setAttribute('y1', this.y0);
            line.setAttribute('x2', this.x0 + x * this.scale);
            line.setAttribute('y2', this.y0 + this.h * this.scale);
            line.setAttribute('stroke', '#8af');
            parent.appendChild(line);
        }

        for (let y = 0; y <= this.h; y++) {
            let line = document.createElementNS(NS, 'line');
            line.setAttribute('x1', this.x0);
            line.setAttribute('y1', this.y0 + y * this.scale);
            line.setAttribute('x2', this.x0 + this.w * this.scale);
            line.setAttribute('y2', this.y0 + y * this.scale);
            line.setAttribute('stroke', '#8af');
            parent.appendChild(line);
        }
    }

    isSet(x, y, isVert) {
        let xy = xy_of(x, y);
        let set = isVert ? this.vset : this.hset;
        return set.has(xy);
    }

    set(x, y, isVert, val) {
        let xy = xy_of(x, y);
        let set = isVert ? this.vset : this.hset;
        let map = isVert ? this.vmap : this.hmap;
        if (val) {
            if (set.has(xy)) { return; }
            set.add(xy);
            let x1 = this.x0 + (x + 0.5) * this.scale;
            let y1 = this.y0 + (y + 0.5) * this.scale;
            let line = document.createElementNS(NS, 'line');
            line.setAttribute('x1', x1);
            line.setAttribute('y1', y1);
            line.setAttribute('x2', isVert ? x1 : x1 + this.scale);
            line.setAttribute('y2', isVert ? y1 + this.scale : y1);
            line.setAttribute('stroke', '#000');
            line.setAttribute('stroke-width', 2);
            this.parent.appendChild(line);
            map[xy] = line;
        } else {
            if (!set.has(xy)) { return; }
            set.delete(xy);
            map[xy].remove();
            map.delete(xy);
        }
    }

    ev_to_xy(ev) {
        let x = Math.floor((ev.offsetX - this.x0) / this.scale);
        let y = Math.floor((ev.offsetY - this.y0) / this.scale);
        if (x >= 0 && x < this.w && y >= 0 && y < this.h) {
            return xy_of(x, y);
        } else {
            return -1;
        }
    }

    onmousedown(ev) {
        let xy = this.ev_to_xy(ev);
        if (xy >= 0) {
            this.clickState = 1;
            this.clickXy = xy;
        }
    }

    onmouseup(ev) {
        this.clickState = 0;
    }

    onmousemove(ev) {
        let xy = this.ev_to_xy(ev);
        if (this.clickState) {
            let seg = this.computeSegment(this.clickXy, xy);
            if (seg) {
                if (this.clickState == 1) {
                    this.clickVal = !this.isSet(seg.x, seg.y, seg.isVert);
                    this.clickState = 2;
                }
                this.set(seg.x, seg.y, seg.isVert, this.clickVal);
                this.clickXy = xy;
            }
        }
    }

    // Note: this really needs to be Bresenham
    computeSegment(xy1, xy2) {
        if (xy1 < 0 || xy2 < 0) { return null; }
        let x1 = x_of(xy1);
        let y1 = y_of(xy1);
        let x2 = x_of(xy2);
        let y2 = y_of(xy2);
        if (x1 == x2) {
            if (y2 == y1 + 1) {
                return {'x': x1, 'y': y1, 'isVert': true};
            } else if (y1 == y2 + 1) {
                return {'x': x1, 'y': y2, 'isVert': true};
            }
        } else if (y1 == y2) {
            if (x2 == x1 + 1) {
                return {'x': x1, 'y': y1, 'isVert': false};
            } else if (x1 == x2 + 1) {
                return {'x': x2, 'y': y1, 'isVert': false};
            }            
        }
        return null;
    }
}

var ui = new Ui(document.getElementById('main'));
ui.init();
