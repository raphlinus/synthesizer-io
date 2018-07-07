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

// create an SVG element with
function svg_el(tag, pe = 'none') {
    let el = document.createElementNS(NS, tag);
    el.setAttribute('pointer-events', pe);
    return el;
}

class Ui {
    constructor(el) {
        this.el = el;
        let grid = new Grid(48, 32);
        this.wireGrid = new WireGrid(grid);
        this.moduleGrid = new ModuleGrid(grid);
        this.handler = this.wireGrid;
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

        this.wireGrid.attach(this.el);
        this.moduleGrid.attach(this.el);

        let rect = this.wireGrid.rect;
        rect.onmousedown = function(ev) {
            self.handler.onmousedown(ev);
            ev.preventDefault();
        }
        rect.onmouseup = function(ev) {
            self.handler.onmouseup(ev);
            ev.preventDefault();
        }
        rect.onmousemove = function(ev) {
            self.handler.onmousemove(ev);
            ev.preventDefault();
        }

        // set up buttons
        let button = new Button(0, 550, 'wire');
        button.onclick = function () {
            self.handler = self.wireGrid;
        };
        button.attach(this.el);
        let button2 = new Button(60, 550, 'mod');
        button2.onclick = function () {
            self.handler = self.moduleGrid;
        };
        button2.attach(this.el);

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

class Grid {
    constructor(w, h) {
        this.w = w;
        this.h = h;
        // use translate in DOM?
        this.x0 = 0;
        this.y0 = 0;
        this.scale = 10;
    }

    x_y_to_xy(x, y) {
        let i = Math.floor(x / this.scale);
        let j = Math.floor(y / this.scale);
        if (i >= 0 && i < this.w && j >= 0 && j < this.h) {
            return xy_of(i, j);
        } else {
            return -1;
        }
    }

    ev_to_xy(ev) {
        return this.x_y_to_xy(ev.offsetX, ev.offsetY);
    }
}

class WireGrid {
    constructor(grid) {
        this.grid = grid;
        this.hset = new Set();
        this.vset = new Set();
        this.hmap = new Map();
        this.vmap = new Map();
        this.clickState = 0;
    }

    attach(parent) {
        this.parent = parent;
        let w = this.grid.w;
        let h = this.grid.h;
        let scale = this.grid.scale;
        let x0 = this.grid.x0;
        let y0 = this.grid.y0;
        let rect = svg_el('rect', 'all');
        rect.setAttribute('x', x0);
        rect.setAttribute('y', y0);
        rect.setAttribute('width', w * scale);
        rect.setAttribute('height', h * scale);
        rect.setAttribute('fill', '#cdf');
        parent.appendChild(rect);

        for (let x = 0; x <= w; x++) {
            let line = svg_el('line');
            line.setAttribute('x1', x0 + x * scale);
            line.setAttribute('y1', y0);
            line.setAttribute('x2', x0 + x * scale);
            line.setAttribute('y2', y0 + h * scale);
            line.setAttribute('stroke', '#8af');
            parent.appendChild(line);
        }

        for (let y = 0; y <= h; y++) {
            let line = svg_el('line');
            line.setAttribute('x1', x0);
            line.setAttribute('y1', y0 + y * scale);
            line.setAttribute('x2', x0 + w * scale);
            line.setAttribute('y2', y0 + y * scale);
            line.setAttribute('stroke', '#8af');
            parent.appendChild(line);
        }
        this.rect = rect;
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
            let x1 = this.grid.x0 + (x + 0.5) * this.grid.scale;
            let y1 = this.grid.y0 + (y + 0.5) * this.grid.scale;
            let line = svg_el('line');
            line.setAttribute('x1', x1);
            line.setAttribute('y1', y1);
            line.setAttribute('x2', isVert ? x1 : x1 + this.grid.scale);
            line.setAttribute('y2', isVert ? y1 + this.grid.scale : y1);
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

    onmousedown(ev) {
        let xy = this.grid.ev_to_xy(ev);
        if (xy >= 0) {
            this.clickState = 1;
            this.clickXy = xy;
        }
    }

    onmouseup(ev) {
        this.clickState = 0;
    }

    onmousemove(ev) {
        let xy = this.grid.ev_to_xy(ev);
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

class Button {
    constructor(x, y, label) {
        let width = 50;
        let rect = svg_el('rect', 'all');
        rect.setAttribute('x', x);
        rect.setAttribute('y', y);
        rect.setAttribute('width', width);
        rect.setAttribute('height', 20);
        rect.setAttribute('fill', '#ddd');
        let text = svg_el('text');
        text.setAttribute('x', x + width / 2);
        text.setAttribute('y', y + 15);
        text.setAttribute('text-anchor', 'middle');
        text.setAttribute('fill', '#000');
        let textNode = document.createTextNode(label);
        text.appendChild(textNode);

        let self = this;
        rect.onclick = function(ev) {
            self.onclick();
        }
        this.onclick = function() { console.log('onclick not set!'); }
        this.rect = rect;
        this.text = text;
    }

    attach(parent) {
        parent.appendChild(this.rect);
        parent.appendChild(this.text);
    }
}

class ModuleGrid {
    constructor(grid) {
        this.grid = grid;
        this.guide = null;
        this.guideWidth = 3;
        this.guideHeight = 3;
        this.modules = [];
    }

    attach(parent) {
        this.parent = parent;
    }

    ev_to_xy(ev) {
        let x0 = ev.offsetX - 0.5 * (this.grid.scale * (this.guideWidth - 1));
        let y0 = ev.offsetY - 0.5 * (this.grid.scale * (this.guideHeight - 1));
        return this.grid.x_y_to_xy(x0, y0);
    }

    xy_ok(xy) {
        if (xy < 0) { return false; }
        let x = x_of(xy);
        let y = y_of(xy);
        if (x + this.guideWidth > this.grid.w) { return false; }
        if (y + this.guideHeight > this.grid.h) { return false; }
        for (let module of this.modules) {
            if (x + this.guideWidth >= x_of(module.xy)
                && x_of(module.xy) + module.w >= x
                && y + this.guideHeight >= y_of(module.xy)
                && y_of(module.xy) + module.h >= y) { return false; }
        }
        return true;
    }

    onmousedown(ev) {
        let xy = this.ev_to_xy(ev);
        if (this.xy_ok(xy)) {
            let rect = svg_el('rect');
            let module = new Module(xy, this.guideWidth, this.guideHeight);
            module.render(this.parent, this.grid);
            this.modules.push(module);
        }
    }

    onmouseup(ev) {
        console.log('mod up');
    }

    onmousemove(ev) {
        let xy = this.ev_to_xy(ev);
        if (xy >= 0) {
            let x0 = this.grid.x0 + x_of(xy) * this.grid.scale;
            let y0 = this.grid.y0 + y_of(xy) * this.grid.scale;
            if (this.guide === null) {
                this.guide = svg_el('rect');
                this.guide.setAttribute('width', this.grid.scale * this.guideWidth);
                this.guide.setAttribute('height', this.grid.scale * this.guideHeight);
                this.guide.setAttribute('fill', '#888');
                this.guide.setAttribute('fill-opacity', 0.5);
                this.guide.setAttribute('stroke', '#888');
                this.parent.appendChild(this.guide)
            }
            if (this.xy_ok(xy)) {
                this.guide.setAttribute('fill', '#0c0');
                this.guide.setAttribute('stroke', '#0c0');
            } else {
                this.guide.setAttribute('fill', '#e00');
                this.guide.setAttribute('stroke', '#e00');
            }
            this.guide.setAttribute('x', x0);
            this.guide.setAttribute('y', y0);
        }
        console.log('mod move');
    }
}

class Module {
    constructor(xy, w, h) {
        this.xy = xy;
        this.w = w;
        this.h = h;
    }

    render(parent, grid) {
        let g = svg_el('g');
        let x = grid.x0 + x_of(this.xy) * grid.scale;
        let y = grid.y0 + y_of(this.xy) * grid.scale;
        g.setAttribute('transform', 'translate(' + x + ' ' + y + ')');
        let rect = svg_el('rect');
        rect.setAttribute('x', 0);
        rect.setAttribute('y', 0);
        rect.setAttribute('width', grid.scale * this.w);
        rect.setAttribute('height', grid.scale * this.h);
        rect.setAttribute('fill', 'none');
        rect.setAttribute('stroke', '#000');
        g.appendChild(rect);

        for (let y = 0; y < this.h; y++) {
            for (let side = 0; side < 2; side++) {
                let line = svg_el('line');
                if (side == 0) {
                    line.setAttribute('x1', -0.5 * grid.scale);
                    line.setAttribute('x2', 0);
                } else {
                    line.setAttribute('x1', this.w * grid.scale);
                    line.setAttribute('x2', (this.w + 0.5) * grid.scale);
                }
                line.setAttribute('y1', grid.scale * (0.5 + y));
                line.setAttribute('y2', grid.scale * (0.5 + y));
                line.setAttribute('stroke', '#000');
                g.appendChild(line);
            }
        }
        this.g = g;
        parent.appendChild(g);
    }
}

var ui = new Ui(document.getElementById('main'));
ui.init();
